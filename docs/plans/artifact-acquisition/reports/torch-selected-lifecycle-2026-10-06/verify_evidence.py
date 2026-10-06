"""Reverify retained synthetic publication evidence; never reconstruct custody."""

import hashlib
import json
from pathlib import Path
import sys
import zipfile


def digest(body):
    return hashlib.sha256(body).hexdigest()


def canonical(value):
    return digest(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    )


def verify(evidence, repository):
    sys.path.insert(0, str(repository / "torch-server"))
    from packaging.utils import canonicalize_name
    from packaging.version import InvalidVersion, Version
    from wheel_records import installed_file_manifest

    def read(name):
        return json.loads((evidence / name).read_bytes())

    record = read("selected-runtime-install.json")
    receipt = read("catalog-receipt.json")
    packet = read("selected-consumption/packet.json")
    profile = read("selected-runtime-profile.json")
    probe = read("probe-results.json")
    target = read("catalog-approved-target.json")
    local = record["local_installation"]
    assert record["schema"] == "pumas.selected-runtime-install.v1"
    assert record["catalog_receipt_sha256"] == canonical(receipt)
    assert record["catalog_acquisition_id"] == receipt["acquisition_id"]
    assert receipt["payload"] == read("catalog-evidence.json")
    assert local["selected_packet_sha256"] == canonical(packet) == profile["selected_packet_sha256"]
    assert (
        local["target_observation_sha256"]
        == canonical(target)
        == profile["target_observation_sha256"]
    )
    assert target == read("selected-target-observation.json")
    assert local["catalog_request_sha256"] == canonical(read("catalog-request.json"))
    assert profile["artifacts"] == packet["selected"]
    assert record["provider"] == profile["provider"] == read("runtime.json")["managed_python"]
    assert record["provider"]["kind"] == "existing-local-fixture-provider"
    assert record["metadata"]["releaseTag"] == record["metadata"]["path"] == "v2.14.0"
    assert record["metadata"]["dependenciesInstalled"] is True
    assert record["metadata"]["downloadUrl"] == next(
        a["url"] for a in packet["selected"] if a["name"] == "torch"
    )
    assert record["probe"] == probe
    assert probe["environment"] == profile
    assert probe["core_status"] == "passed"
    assert probe["capabilities"]["cpu_tensor"]["status"] == "passed"
    assert probe["capabilities"]["sidecar_app"]["health"] == {"protocol": 3, "status": "ok"}
    assert probe["capabilities"]["image_generation"]["status"] == "not tested"
    assert probe["context"]["runtime_profile_sha256"] == digest(
        (evidence / "selected-runtime-profile.json").read_bytes()
    )
    assert (
        probe["context"]["interpreter_sha256"]
        == local["interpreter_sha256"]
        == target["interpreter_sha256"]
    )
    for source in ("serve.py", "probe_runtime.py"):
        assert probe["context"]["runtime_files_sha256"][source] == digest(
            (evidence / source).read_bytes()
        )
    for field, name in {
        "installation_report_sha256": "local-pip-report.json",
        "local_requirements_sha256": "local-requirements.txt",
        "installed_manifest_sha256": "installed-files.json",
    }.items():
        assert local[field] == digest((evidence / "selected-consumption/proof" / name).read_bytes())
    assert packet["lock_sha256"] == digest(
        (evidence / "offline-selection/pylock.toml").read_bytes()
    )
    assert packet["projection_sha256"] == canonical(read("offline-selection/projection.json"))
    manifest = installed_file_manifest(
        evidence / "installed",
        packet["selected"],
        canonicalize_name=canonicalize_name,
        Version=Version,
        InvalidVersion=InvalidVersion,
    )
    assert manifest == read("selected-consumption/proof/installed-files.json")
    assert len(manifest["files"]) == local["installed_files"] == 21
    for row in packet["selected"]:
        wheel = evidence / "wheels" / row["filename"]
        assert wheel.stat().st_size == row["size"]
        assert digest(wheel.read_bytes()) == row["sha256"]
        with zipfile.ZipFile(wheel) as archive:
            assert archive.testzip() is None
        assert probe["context"]["installed_distributions"][row["name"]] == row["version"]
    return {
        "catalog_receipt_unchanged": True,
        "selected_wheels": len(packet["selected"]),
        "installed_members": len(manifest["files"]),
        "record_profile_probe_binding": "verified",
        "custody_reconstruction": False,
        "real_provider_qualification": False,
    }


if __name__ == "__main__":
    evidence = (
        Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "publication"
    )
    repository = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).resolve().parents[5]
    print(json.dumps(verify(evidence, repository), indent=2))
