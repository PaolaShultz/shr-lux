#!/usr/bin/env python3
"""Real GP04 -> Lux LX05 private synthetic process check. Never opens devices."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time
from importlib.util import spec_from_file_location, module_from_spec

spec = spec_from_file_location("lux_cli", Path(__file__).with_name("check-lux-service.py"))
cli = module_from_spec(spec)
spec.loader.exec_module(cli)


def checksum(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(provider, lux):
    with tempfile.TemporaryDirectory(prefix="lux-analysis-") as name:
        root = Path(name).resolve()
        source, owner = root / "source", root / "lux"
        source.mkdir(mode=0o700)
        owner.mkdir(mode=0o700)
        children = []
        client = None
        try:
            producer = subprocess.Popen([str(provider), "--directory", str(source),
                "--show", cli.SHOW, "--epoch", "9", "--synthetic-source", "fouraux",
                "--analysis"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE)
            children.append(producer)
            for _ in range(300):
                if producer.poll() is not None:
                    raise RuntimeError(producer.stderr.read().decode())
                if (source / "analysis.sock").exists():
                    break
                time.sleep(.01)
            else:
                raise RuntimeError("analysis provider startup deadline")
            consumer = subprocess.Popen([str(lux), "--synthetic-private-dir", str(owner),
                "--timed", "--analysis", str(source / "analysis.sock")],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            children.append(consumer)
            for _ in range(300):
                if consumer.poll() is not None:
                    raise RuntimeError(consumer.stderr.read().decode())
                if (owner / "lux.sock").exists():
                    break
                time.sleep(.01)
            else:
                raise RuntimeError("Lux startup deadline")
            client = cli.Client(owner)
            client.write("grant", dict(scope=cli.SCOPE))
            renew_at = time.monotonic() + .5

            def snapshot():
                nonlocal renew_at
                inv = client.snapshot()
                if time.monotonic() >= renew_at:
                    client.write("renew", dict(scope=cli.SCOPE))
                    renew_at = time.monotonic() + .5
                return inv

            def wait(predicate, limit=3):
                deadline = time.monotonic() + limit
                while time.monotonic() < deadline:
                    inv = snapshot()
                    if predicate(inv):
                        return inv
                    time.sleep(.01)
                raise AssertionError(inv)

            def action(command):
                # Authority steps may advance revision between snapshot and command.
                for _ in range(20):
                    snapshot()
                    request = client.envelope("command", dict(scope=cli.SCOPE, command=command), True)
                    reply = client.exchange(request)[0]
                    client.next_id += 1
                    client.revision = reply["revision"]
                    if reply["kind"] == "applied":
                        return reply
                    assert reply["reason"] == "stale_revision", reply
                raise AssertionError("bounded command contention")

            attached = wait(lambda inv: inv["analysis"]["source_identity"] is not None)
            assert attached["wire_schema"] == "lx05-v1"
            assert attached["analysis"]["source_identity"]["sample_rate"] == 48000
            action(dict(action="analysis_calibrate", phase="start"))
            wait(lambda inv: inv["analysis"]["calibration_windows"] >= 60)
            action(dict(action="analysis_calibrate", phase="finish"))
            ready = wait(lambda inv: inv["analysis"]["state"] == "ready")
            assert ready["analysis"]["confidence"] == 1000
            assert ready["analysis"]["source_age_ms"] <= 100
            assert ready["analysis"]["energy_millionths"] > 0
            assert ready["analysis"]["beat"] is None and ready["analysis"]["downbeat"] is None
            action(dict(action="mode", mode="auto"))
            action(dict(action="analysis_grant", fixtures=["fixture-11"], cap=500, ttl_ms=1000))
            active = wait(lambda inv: inv["snapshot"]["fixtures"][0]["attributes"][0]["resolved"] > 0)
            assert active["snapshot"]["fixtures"][0]["attributes"][0]["source"] == {"auto":"analysis-active"}
            producer.kill()
            producer.wait(timeout=5)
            lost = wait(lambda inv: inv["analysis"]["grant"] is None)
            retained = lost["snapshot"]["fixtures"][0]["attributes"][0]
            assert retained["source"] == {"auto":"analysis-held"}
            time.sleep(.12)
            again = snapshot()["snapshot"]["fixtures"][0]["attributes"][0]
            assert again["resolved"] == retained["resolved"]
            action(dict(action="touch", values=[dict(fixture="fixture-11", attribute="intensity", value=900)]))
            manual = snapshot()
            assert manual["snapshot"]["fixtures"][0]["attributes"][0]["resolved"] == 900
            assert manual["snapshot"]["fixtures"][0]["attributes"][0]["source"] == "programmer"
            return dict(state="passed", provider_sha256=checksum(provider), lux_sha256=checksum(lux),
                descriptor=ready["analysis"]["source_identity"], ready=ready["analysis"],
                active=active, lost=lost, manual=manual, commands=client.log,
                limits=["synthetic same-host private UDS", "null output", "no hardware/load/playback"])
        finally:
            if client is not None:
                client.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=5)
                if child.stderr:
                    child.stderr.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", type=Path, required=True)
    parser.add_argument("--lux", type=Path, default=Path("target/release/lux-service"))
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    result = run(args.provider.resolve(), args.lux.resolve())
    args.evidence.write_text(json.dumps(result, indent=2) + "\n")
    print("PASS real synthetic GP04/LX05 analysis, explicit calibration/grant, loss retention, manual independence")


if __name__ == "__main__":
    main()
