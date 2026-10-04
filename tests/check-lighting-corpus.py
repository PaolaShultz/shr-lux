#!/usr/bin/env python3
"""Fast offline consistency check; Rust tests exercise actual owner validation."""
import csv
import hashlib
import json
from pathlib import Path

root = Path(__file__).parent / "fixtures" / "c-light-v1"
raw = (root / "e04.json").read_bytes()
assert len(raw) <= 65536


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, f"Duplicate key {key}"
        result[key] = value
    return result


def check_depth(value, depth=1):
    assert depth <= 12
    if isinstance(value, dict):
        for child in value.values():
            check_depth(child, depth + 1)
    elif isinstance(value, list):
        for child in value:
            check_depth(child, depth + 1)


def typed(value):
    if value == "null":
        return None
    if value in ("true", "false"):
        return value == "true"
    if value.lstrip("-").isdigit():
        return int(value)
    return value


doc = json.loads(raw, object_pairs_hook=unique_object)
check_depth(doc)
assert doc["contract"] == "C-LIGHT" and doc["version"] == 1
assert doc["rig"]["synthetic"] is True
assert doc["status"] == "expected_states_validated_not_authority_execution"
with (root / "e04.tsv").open() as source:
    rows = [{k: typed(v) for k, v in row.items()}
            for row in csv.DictReader(source, delimiter="\t")]
assert doc["cases"] == rows
assert len(rows) == 5
for row in rows:
    assert row["submitted"] is None and row["observed"] is None
    assert row["command"] and row["prior_state"]
patch = json.loads((root / "patch.json").read_bytes(), object_pairs_hook=unique_object)
check_depth(patch)
assert patch["contract"] == "C-LIGHT" and patch["version"] == 1
assert patch["patch_revision"] == doc["rig"]["patch_revision"]
fixture, = patch["fixtures"]
assert fixture["id"] == doc["rig"]["fixture_id"]
assert fixture["mode"] == doc["rig"]["mode"] and fixture["synthetic"] is True
assert fixture["address"] == doc["rig"]["address"] and fixture["footprint"] == 7
assert fixture["coherent_groups"] == [["red", "green", "blue"], ["pan", "tilt"]]
assert [c["attribute"] for c in fixture["capabilities"]] == [
    "intensity", "red", "green", "blue", "pan", "tilt", "zoom"]
for cap in fixture["capabilities"]:
    expected = {"pan": (-2700, 2700, 0), "tilt": (-1350, 1350, 0),
                "zoom": (50, 450, 50)}.get(cap["attribute"], (0, 1000, 0))
    assert (cap["min"], cap["max"], cap["default"]) == expected
    assert cap["unit"] == ("tenth_degree" if cap["attribute"] in
                           ("pan", "tilt", "zoom") else "tenth_percent")
for line in (root / "SHA256SUMS").read_text().splitlines():
    digest, name = line.split("  ", 1)
    assert Path(name).name == name
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == digest, name
print("PASS: E04 JSON/TSV parity, bounds, provenance and fixture hashes")
