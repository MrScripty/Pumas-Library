"""Verify retained synthetic staged-install evidence; no environment/network access."""
import hashlib
import json
from pathlib import Path
import tomllib

root = Path(__file__).resolve().parent

def load(name):
    return json.loads((root / name).read_bytes())

def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

packet, report, proof = load("packet.json"), load("installation-report.json"), load("installation-proof.json")
request, target, catalog = load("request.json"), load("target.json"), load("catalog.json")
assert proof["schema"] == "pumas.selected-local-install.v1"
assert packet["request_sha256"] == canonical(request) == proof["catalog_request_sha256"]
assert packet["catalog_sha256"] == canonical(catalog)
assert packet["target_observation_sha256"] == canonical(target) == proof["target_observation_sha256"]
assert proof["selected_packet_sha256"] == canonical(packet)
assert proof["interpreter_sha256"] == target["interpreter_sha256"]
assert report["version"] == "1" and report["environment"] == target["target"]["markers"]
expected = {c["name"]: (c["version"], Path(c["local"]).as_uri(), c["sha256"]) for c in packet["selected"]}
assert len(expected) == len(packet["selected"]) == 3
assert len(catalog["candidates"]) == 5
for c in packet["selected"]:
    assert {k:v for k,v in c.items() if k != "local"} in catalog["candidates"]
    assert c["url"].startswith("https://")
observed = {}
for item in report["install"]:
    assert item["is_direct"] is True and item["requested"] is True and item["is_yanked"] is False
    name = item["metadata"]["name"]
    assert name not in observed
    observed[name] = (item["metadata"]["version"], item["download_info"]["url"], item["download_info"]["archive_info"]["hashes"]["sha256"])
assert observed == expected
lock_bytes = (root / "pylock.toml").read_bytes()
assert hashlib.sha256(lock_bytes).hexdigest() == packet["lock_sha256"]
lock = tomllib.loads(lock_bytes.decode())
assert lock["created-by"] == "uv" and lock["lock-version"] == "1.0"
assert {p["name"] for p in lock["packages"]} == set(expected)
for name, field in (("installation-report.json", "installation_report_sha256"),
                    ("installed-files.json", "installed_manifest_sha256"),
                    ("local-requirements.txt", "local_requirements_sha256")):
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == proof[field]
files = load("installed-files.json")["files"]
assert len(files) == proof["installed_files"] == 21
assert len({f["path"] for f in files}) == len(files)
assert len([f for f in files if f["path"].endswith(".dist-info/RECORD")]) == 3
assert all(f["size"] >= 0 and len(f["sha256"]) == 64 for f in files)
print("PASS: genuine report/selected catalog/local paths/target/lock/proof identity; 3 selected, 5 acquired, 21 installed members")
print("Retained digests are evidence; original temporary installed files are not retained or cold-reopened as capabilities.")
