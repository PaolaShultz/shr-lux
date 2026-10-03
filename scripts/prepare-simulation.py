#!/usr/bin/env python3
"""Prepare two private AUX replays. Requires Python 3, SoX; curl for --download.

Only metadata/code is versioned. Source archives, extracted files and derivatives
stay under recordings/. Never strip leading silence or independently align tracks.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tempfile
import wave
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CHANNELS = ("kick", "bass", "guitar_1", "guitar_2")
RATE = 48_000


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(*args):
    subprocess.run([str(a) for a in args], check=True, timeout=600)


def wave_info(path):
    with wave.open(str(path), "rb") as wav:
        result = {"channels": wav.getnchannels(), "sample_rate": wav.getframerate(),
                  "sample_width": wav.getsampwidth(), "frames": wav.getnframes()}
        if wav.getcomptype() != "NONE" or not result["frames"]:
            raise ValueError(f"Expected nonempty PCM: {path}")
        # Catch headers that claim more audio than actually exists.
        remaining = result["frames"]
        while remaining:
            count = min(remaining, 8192)
            if len(wav.readframes(count)) != count * result["channels"] * result["sample_width"]:
                raise ValueError(f"Truncated PCM: {path}")
            remaining -= count
        return result


def pad_tail(source, destination, frames):
    """Append zeros only; never move, stretch or remove samples."""
    with wave.open(str(source), "rb") as src:
        if frames < src.getnframes():
            raise ValueError("Tail padding cannot shorten audio")
        with wave.open(str(destination), "wb") as dst:
            dst.setparams(src.getparams())
            while data := src.readframes(8192):
                dst.writeframesraw(data)
            remaining = frames - src.getnframes()
            width = src.getnchannels() * src.getsampwidth()
            while remaining:
                count = min(remaining, 8192)
                dst.writeframesraw(bytes(count * width))
                remaining -= count


def extract_member(archive, name, destination):
    """Extract one explicitly mapped regular file to a caller-owned path."""
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or "\\" in name:
        raise ValueError("Unsafe archive member")
    info = archive.getinfo(name)
    if info.is_dir() or info.file_size > 512 * 1024 * 1024:
        raise ValueError("Unexpected source member size/type")
    with archive.open(info) as src, destination.open("wb") as dst:
        shutil.copyfileobj(src, dst, 1024 * 1024)


def verify_prepared(directory, fingerprint):
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest["recipe_sha256"] != fingerprint:
        raise ValueError(f"Recipe changed. Preserve/remove {directory} before rebuilding.")
    for filename, expected in manifest["files_sha256"].items():
        if Path(filename).name != filename or digest(directory / filename) != expected:
            raise ValueError(f"Prepared file changed: {filename}")
    return manifest


def prepare(key, song, download, sox):
    recordings = ROOT / "recordings"
    cache = recordings / "downloads"
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / (song["archive_directory"] + ".zip")
    fingerprint = hashlib.sha256(
        json.dumps(song, sort_keys=True).encode() + Path(__file__).read_bytes() + sox.encode()
    ).hexdigest()
    target = recordings / "simulation" / key
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        report = verify_prepared(target, fingerprint)
        print(f"{key}: verified existing {report['frames'] / RATE:.2f}s replay: {target}", flush=True)
        return
    if not archive.exists():
        if not download:
            raise ValueError(f"Missing {archive.name}; rerun with --download (about 734 MiB for both songs)")
        if not shutil.which("curl"):
            raise ValueError("curl is required for --download")
        print(f"Downloading {song['artist']} — {song['title']} for local educational use", flush=True)
        partial = archive.with_suffix(".zip.part")
        run("curl", "--fail", "--location", "--silent", "--show-error", "--max-time", "600",
            "--max-filesize", str(900 * 1024 * 1024), "--output", partial, song["archive_url"])
        if digest(partial) != song["archive_sha256"]:
            raise ValueError(f"Download checksum differs: {partial}; source may have changed")
        partial.rename(archive)
    if digest(archive) != song["archive_sha256"]:
        raise ValueError(f"Source checksum differs: {archive}")
    if set(song["channels"]) != set(CHANNELS):
        raise ValueError("Recipe must contain all four AUX roles")
    with tempfile.TemporaryDirectory(prefix=key + "-", dir=target.parent) as temporary:
        stage = Path(temporary)
        raw = stage / "raw"
        raw.mkdir()
        converted = stage / "converted"
        converted.mkdir()
        source_info = {}
        lengths = []
        with zipfile.ZipFile(archive) as zip_file:
            folder = song["archive_directory"]
            extract_member(zip_file, folder + "/Readme.txt", stage / "SOURCE-README.txt")
            for role in CHANNELS:
                source = raw / (role + ".wav")
                extract_member(zip_file, folder + "/" + song["channels"][role], source)
                info = wave_info(source)
                if info["channels"] != 1:
                    raise ValueError("This recipe expects mono sources; review stereo downmix explicitly")
                source_info[role] = {**info, "member": song["channels"][role], "sha256": digest(source)}
                # Fixed shared gain, no normalization/compression/gating/time edits.
                # -D disables random dither; -R makes the SoX run repeatable.
                run("sox", "-R", "-D", "-v", "0.5", source, "-t", "wavpcm", "-b", "16",
                    converted / (role + ".wav"), "rate", "-v", str(RATE))
                lengths.append(wave_info(converted / (role + ".wav"))["frames"])
        frames = max(lengths)
        for role in CHANNELS:
            pad_tail(converted / (role + ".wav"), stage / (role + ".wav"), frames)
        # Explicit unit gain preserves each prepared channel exactly.
        inputs = [arg for role in CHANNELS for arg in ("-v", "1", stage / (role + ".wav"))]
        run("sox", "-R", "-D", "-M", *inputs, "-t", "wavpcm", "-b", "16", stage / "aux.wav")
        # Listening aid containing only these AUXes, not the original full-band mix.
        run("sox", "-R", "-D", stage / "aux.wav", "-t", "wavpcm", "-b", "16", stage / "monitor.wav",
            "remix", "1v0.25,2v0.25,3v0.5", "1v0.25,2v0.25,4v0.5")
        for role in CHANNELS + ("aux", "monitor"):
            info = wave_info(stage / (role + ".wav"))
            channels = 4 if role == "aux" else 2 if role == "monitor" else 1
            if info != {"channels": channels, "sample_rate": RATE, "sample_width": 2, "frames": frames}:
                raise ValueError(f"Prepared audio contract failed: {role}: {info}")
        report = {"schema_version": 1, "artist": song["artist"], "title": song["title"],
                  "source_page": song["source_page"], "archive_sha256": song["archive_sha256"],
                  "recipe_sha256": fingerprint, "channel_order": CHANNELS, "sample_rate": RATE,
                  "frames": frames, "gain": 0.5, "source_info": source_info,
                  "tail_padding_frames": dict(zip(CHANNELS, (frames - n for n in lengths))),
                  "sox": sox, "usage": "Private educational experiment only; do not redistribute audio."}
        shutil.rmtree(raw)
        shutil.rmtree(converted)
        report["files_sha256"] = {p.name: digest(p) for p in sorted(stage.iterdir()) if p.is_file()}
        (stage / "manifest.json").write_text(json.dumps(report, indent=2) + "\n")
        stage.rename(target)
        print(f"{key}: prepared {frames / RATE:.2f}s, four AUX tracks, aux.wav and monitor.wav: {target}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--song", choices=["punk", "metal", "all"], default="all")
    parser.add_argument("--download", action="store_true", help="fetch missing original archives")
    args = parser.parse_args()
    if not shutil.which("sox"):
        parser.error("SoX is required on the preparation machine (Debian/Raspberry Pi OS: apt install sox)")
    catalog = json.loads((ROOT / "simulation/songs.json").read_text())
    if catalog["schema_version"] != 1:
        parser.error("Unsupported catalog version")
    sox = subprocess.check_output(["sox", "--version"], text=True).strip()
    try:
        for key, song in catalog["songs"].items():
            if args.song in (key, "all"):
                prepare(key, song, args.download, sox)
    except (OSError, ValueError, KeyError, wave.Error, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        parser.exit(1, f"Preparation failed: {error}\n")


if __name__ == "__main__":
    main()
