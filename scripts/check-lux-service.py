#!/usr/bin/env python3
"""Bounded real Lux null-output CLI E05/restart check; no production endpoints."""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

SHOW = "11111111-1111-4111-8111-111111111111"
SCOPE = "lighting-control"


def exact(stream, count):
    parts = bytearray()
    while len(parts) < count:
        chunk = stream.recv(count - len(parts))
        if not chunk:
            raise RuntimeError("provider closed frame")
        parts.extend(chunk)
    return bytes(parts)


def receive(stream):
    size, = struct.unpack("!I", exact(stream, 4))
    assert 0 < size <= 65536
    return json.loads(exact(stream, size))


class Client:
    def __init__(self, directory):
        self.stream = socket.socket(socket.AF_UNIX)
        self.stream.settimeout(3)
        self.stream.connect(str(directory / "lux.sock"))
        self.epoch = "0"
        self.revision = "0"
        self.writer = "demo-writer"
        self.lease = None
        self.next_id = 1
        self.log = []
        # A deliberate old epoch probe obtains the current owner epoch without granting.
        probe = self.exchange(self.envelope("snapshot", {}))[0]
        assert probe["kind"] == "rejected" and probe["reason"] == "epoch"
        self.epoch = probe["epoch"]
        self.snapshot()

    def envelope(self, kind, body, identity=False):
        return dict(contract="C-LIGHT", version=1, show_id=SHOW,
                    module="lighting", epoch=self.epoch,
                    writer=self.writer if identity else None,
                    lease=self.lease if identity else None,
                    request_id=str(self.next_id) if identity else None,
                    expected_revision=self.revision if identity else None,
                    kind=kind, body=body)

    def exchange(self, request):
        data = json.dumps(request, separators=(",", ":")).encode()
        assert len(data) <= 65536
        self.stream.sendall(struct.pack("!I", len(data)) + data)
        first = receive(self.stream)
        replies = [first]
        if first["kind"] == "snapshot":
            count = first["body"]["page_count"]
            assert 1 <= count <= 16
            replies.extend(receive(self.stream) for _ in range(count - 1))
        self.log.append(dict(input=request, replies=replies))
        return replies

    def snapshot(self):
        pages = self.exchange(self.envelope("snapshot", {}))
        assert all(p["kind"] == "snapshot" for p in pages)
        key = tuple(pages[0][k] for k in ("show_id", "epoch", "revision", "sequence"))
        assert all(tuple(p[k] for k in ("show_id", "epoch", "revision", "sequence")) == key for p in pages)
        assert [p["body"]["page"] for p in pages] == list(range(len(pages)))
        inventory = json.loads("".join(p["body"]["chunk"] for p in pages))
        assert inventory["output"] == "null_disarmed" and inventory["physical"] == "unknown"
        assert inventory["snapshot"]["epoch"] == self.epoch
        self.revision = inventory["snapshot"]["revision"]
        return inventory

    def write(self, kind, body):
        request = self.envelope(kind, body, True)
        reply = self.exchange(request)[0]
        self.next_id += 1
        assert reply["kind"] == "applied", reply
        self.revision = reply["revision"]
        if kind == "grant":
            self.lease = reply["body"]["lease"]
        return request, reply

    def command(self, action):
        return self.write("command", dict(scope=SCOPE, command=action))

    def close(self):
        self.stream.close()


def run(binary):
    with tempfile.TemporaryDirectory(prefix="lux-cli-demo-") as name:
        directory = Path(name).resolve()
        children = []

        def start():
            child = subprocess.Popen([str(binary), "--synthetic-private-dir", str(directory), "--durable"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            children.append(child)
            for _ in range(200):
                if child.poll() is not None:
                    raise RuntimeError(child.stderr.read().decode())
                if (directory / "lux.sock").exists():
                    return child, Client(directory)
                time.sleep(.01)
            raise RuntimeError("bounded startup timeout")

        def crash(child, client):
            client.close()
            child.kill()
            child.wait(timeout=5)
            # Only the socket created by this owned child, after its exit.
            (directory / "lux.sock").unlink()

        try:
            child, client = start()
            client.write("grant", dict(scope=SCOPE))
            value = lambda level: dict(fixture="fixture-11", attribute="intensity", value=level)
            for action in [dict(action="touch", values=[value(700)]),
                           dict(action="record", kind="cue", id="demo-cue"),
                           dict(action="go", cue="demo-cue", playback="demo-playback"),
                           dict(action="touch", values=[value(0)]),
                           dict(action="clear_to_hold")]:
                client.command(action)
            _, preview = client.command(dict(action="release_preview", values=[value(0)]))
            old_commit, original = client.command(dict(action="release_commit", token=preview["body"]["token"]))
            start_tick = int(original["effective_tick"])
            seen = []
            for _ in range(100):
                inventory = client.snapshot()
                attr = inventory["snapshot"]["fixtures"][0]["attributes"][0]
                transition = inventory["release"]["transition"]
                seen.append(dict(revision=inventory["snapshot"]["revision"], value=attr["resolved"], source=attr["source"], hold=attr["hold"], transition=transition))
                if transition is None:
                    assert attr["resolved"] == 700 and attr["hold"] is None
                    break
                assert attr["hold"] is None and attr["source"] == "release"
                time.sleep(.02)
            else:
                raise AssertionError("bounded fade did not finish")
            assert any(0 < row["value"] < 700 for row in seen)
            newer = client.revision
            assert client.exchange(old_commit)[0] == original
            assert client.snapshot()["snapshot"]["revision"] == newer
            for action in [dict(action="touch", values=[value(333)]),
                           dict(action="clear_to_hold"), dict(action="master", level=500),
                           dict(action="blackout", enabled=True), dict(action="checkpoint")]:
                client.command(action)
            before = client.snapshot()
            assert before["snapshot"]["durability"] == "checkpointed"
            logs = [client.log]
            crash(child, client)
            epochs = [before["snapshot"]["epoch"]]
            for _ in range(2):
                child, client = start()
                after = client.snapshot()
                epochs.append(client.epoch)
                assert int(epochs[-1]) == int(epochs[-2]) + 1
                assert after["snapshot"]["fixtures"][0]["attributes"][0]["hold"] == 333
                assert after["master"] == 500 and after["blackout"] is True
                assert after["authority_inventory"]["playbacks"] == [] and after["grants"] == []
                assert after["release"]["transition"] is None
                assert client.exchange(old_commit)[0]["reason"] == "epoch"
                logs.append(client.log)
                crash(child, client)
            return dict(status="passed", binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), epochs=epochs, start_tick=start_tick, release_samples=seen, checkpointed=before, recovered=after, exchanges=logs)
        finally:
            for child in children:
                if child.poll() is None:
                    child.kill()
                child.wait(timeout=5)
                child.stderr.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/lux-service"))
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    result = run(args.binary.resolve(strict=True))
    if args.evidence:
        args.evidence.write_text(json.dumps(result, indent=2) + "\n")
    print("PASS: actual CLI E05 fade, immutable retry, checkpoint, two crashed disarmed restarts; epochs " + ",".join(result["epochs"]))


if __name__ == "__main__":
    main()
