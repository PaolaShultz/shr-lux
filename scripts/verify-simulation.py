#!/usr/bin/env python3
"""Opt-in complete-song audit. Requires locally prepared audio; never downloads it."""
import array
import json
from pathlib import Path
import sys
import wave

ROOT = Path(__file__).resolve().parents[1]


def pcm(data):
    values = array.array("h", data)
    if sys.byteorder != "little":
        values.byteswap()
    return values


def audit(song):
    root = ROOT / "recordings/simulation" / song
    manifest = json.loads((root / "manifest.json").read_text())
    from contextlib import ExitStack
    peaks = [0] * 4
    nonzero = [0] * 4
    clipped = [0] * 4
    frames = 0
    distinct_guitars = False
    with ExitStack() as stack:
        readers = [stack.enter_context(wave.open(str(root / (role + ".wav"))))
                   for role in manifest["channel_order"]]
        merged = stack.enter_context(wave.open(str(root / "aux.wav")))
        if merged.getnchannels() != 4 or merged.getsampwidth() != 2 or merged.getframerate() != 48000:
            raise ValueError("Expected canonical four-channel audio")
        while chunk := merged.readframes(8192):
            data = pcm(chunk)
            count = len(data) // 4
            distinct_guitars |= data[2::4] != data[3::4]
            for index, reader in enumerate(readers):
                source = pcm(reader.readframes(count))
                if data[index::4] != source:
                    raise ValueError(f"{song}: channel mismatch at frame {frames}, channel {index}")
                peaks[index] = max(peaks[index], max((abs(v) for v in source), default=0))
                nonzero[index] += sum(v != 0 for v in source)
                clipped[index] += sum(abs(v) >= 32767 for v in source)
            frames += count
        if any(reader.readframes(1) for reader in readers) or frames != manifest["frames"]:
            raise ValueError(f"{song}: inconsistent song lengths")
    if not all(nonzero) or any(clipped):
        raise ValueError(f"{song}: silent or clipped prepared channel")
    if not distinct_guitars:
        raise ValueError(f"{song}: guitar channels contain identical samples")
    return {"frames": frames, "duration_s": frames / 48000,
            "peak_pcm16": peaks, "nonzero_samples": nonzero, "clipped_samples": clipped,
            "tail_padding_frames": manifest["tail_padding_frames"],
            "all_interleaved_samples_match": True}


def main():
    results = {}
    for song in ("punk", "metal"):
        results[song] = audit(song)
        print(f"{song}: PASS; {results[song]['frames']} frames, all four channels match, no clipping", flush=True)
    output = ROOT / "local/simulation-validation.json"
    output.parent.mkdir(exist_ok=True)
    output.write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
