#!/usr/bin/env python3
"""Fast preparation contract tests. Synthetic audio only; no SoX/network required."""
import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import wave
import zipfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("prepare", Path(__file__).with_name("prepare-simulation.py"))
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def audio(self, name="in.wav", samples=(0, 123, -456, 0)):
        path = self.root / name
        with wave.open(str(path), "wb") as wav:
            wav.setparams((1, 2, 48_000, 0, "NONE", "not compressed"))
            wav.writeframes(struct.pack("<" + "h" * len(samples), *samples))
        return path

    def test_padding_keeps_leading_silence_and_samples(self):
        source = self.audio()
        out = self.root / "padded.wav"
        prepare.pad_tail(source, out, 7)
        self.assertEqual(prepare.wave_info(out)["frames"], 7)
        with wave.open(str(out)) as wav:
            self.assertEqual(struct.unpack("<7h", wav.readframes(7)), (0, 123, -456, 0, 0, 0, 0))

    def test_padding_cannot_truncate(self):
        with self.assertRaises(ValueError):
            prepare.pad_tail(self.audio(), self.root / "out.wav", 1)

    def test_truncated_and_empty_audio_are_rejected(self):
        path = self.audio()
        path.write_bytes(path.read_bytes()[:-1])
        with self.assertRaises(ValueError):
            prepare.wave_info(path)
        with self.assertRaises(ValueError):
            prepare.wave_info(self.audio("empty.wav", ()))

    def test_extract_only_explicit_member_to_chosen_destination(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("song/readme.txt", "educational")
            archive.writestr("../escape", "unwanted")
        data.seek(0)
        with zipfile.ZipFile(data) as archive:
            dest = self.root / "readme.txt"
            prepare.extract_member(archive, "song/readme.txt", dest)
            self.assertEqual(dest.read_text(), "educational")
            with self.assertRaises(ValueError):
                prepare.extract_member(archive, "../escape", dest)
            with self.assertRaises(KeyError):
                prepare.extract_member(archive, "missing.wav", dest)

    def test_cache_detects_changed_recipe_or_audio(self):
        path = self.audio()
        (self.root / "manifest.json").write_text(json.dumps({
            "recipe_sha256": "recipe", "files_sha256": {path.name: prepare.digest(path)}}))
        prepare.verify_prepared(self.root, "recipe")
        with self.assertRaises(ValueError):
            prepare.verify_prepared(self.root, "different")
        path.write_bytes(b"changed")
        with self.assertRaises(ValueError):
            prepare.verify_prepared(self.root, "recipe")

    def test_catalog_maps_four_distinct_recorded_sources_per_song(self):
        catalog = json.loads((prepare.ROOT / "simulation/songs.json").read_text())
        self.assertEqual(set(catalog["songs"]), {"punk", "metal"})
        for song in catalog["songs"].values():
            self.assertEqual(set(song["channels"]), set(prepare.CHANNELS))
            self.assertEqual(len(set(song["channels"].values())), 4)
            self.assertEqual(len(song["archive_sha256"]), 64)


if __name__ == "__main__":
    unittest.main()
