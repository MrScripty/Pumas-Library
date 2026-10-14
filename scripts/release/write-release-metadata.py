#!/usr/bin/env python3
"""Describe every final candidate artifact; never grant publication acceptance."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_metadata(directory, build_id, source, repository=ROOT):
    directory, repository = Path(directory), Path(repository)
    version = json.loads((repository / "package.json").read_text())["version"]
    plan = json.loads((repository / "scripts/release/artifact-plan.json").read_text())
    artifacts = {
        entry["filename"].replace("{version}", version): entry for entry in plan["artifacts"]
    }
    # The existing checker has already collected the complete flat artifact set.
    # Reject missing/foreign files here too before emitting partial descriptions.
    present = {p.name for p in directory.iterdir() if p.name != "checksums-sha256.txt"}
    if present != set(artifacts):
        raise ValueError("metadata requires the exact fresh artifact-plan inventory")
    for name in artifacts:
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError("metadata requires regular nonempty final artifacts")
    inventories, notices = {}, []
    for profile in ("", "-s3"):
        base = repository / f"docs/release-attribution/{version}{profile}"
        inventory = json.loads((base / "inventory.json").read_text())
        for name, expected in inventory["input_sha256"].items():
            if digest(repository / name) != expected:
                raise ValueError("release attribution inputs changed")
        text = base / "THIRD-PARTY-NOTICES.txt"
        if digest(text) != inventory["notices_sha256"]:
            raise ValueError("release attribution text changed")
        inventories[profile] = inventory
        notices.append(text.read_bytes())
    pins = json.loads((repository / "scripts/release/onnx-runtime-pins.json").read_text())
    for name, member in (
        ("onnxruntime-LICENSE", "ONNX-LICENSE.txt"),
        ("onnxruntime-ThirdPartyNotices.txt", "ONNX-NOTICES.txt"),
    ):
        path = repository / "scripts/release/licenses" / name
        expected = {
            (target["notices"][member]["bytes"], target["notices"][member]["sha256"])
            for target in pins["targets"].values()
        }
        # Upstream Windows uses CRLF for these same texts. Emit only exact
        # byte representations independently authenticated by every target pin;
        # never assume one target's notice covers another target's notice role.
        lf = path.read_bytes().replace(b"\r\n", b"\n")
        variants = {
            (len(data), hashlib.sha256(data).hexdigest()): data
            for data in (lf, lf.replace(b"\n", b"\r\n"))
        }
        if not expected <= variants.keys():
            raise ValueError("native notice differs from reviewed runtime authority")
        notices.extend(variants[key] for key in sorted(expected))
    created = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    subjects = []
    targets = {entry["id"]: entry for entry in plan["desktop_targets"] + plan["headless_targets"]}
    for filename, artifact in sorted(artifacts.items()):
        sha = digest(directory / filename)
        subjects.append({"name": filename, "digest": {"sha256": sha}})
        profile = "-s3" if artifact.get("variant") == "headless-inference" else ""
        packages = [
            {
                "name": "Pumas Library",
                "SPDXID": "SPDXRef-Pumas",
                "versionInfo": version,
                "downloadLocation": "NOASSERTION",
                "filesAnalyzed": False,
                "licenseDeclared": "MIT",
                "licenseConcluded": "NOASSERTION",
                "copyrightText": "NOASSERTION",
                "checksums": [{"algorithm": "SHA256", "checksumValue": sha}],
                "sourceInfo": f"Artifact {artifact['id']}; target {artifact['target']}; build {build_id}",
            }
        ]
        relationships = [
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": "SPDXRef-Pumas",
            }
        ]
        if artifact.get("variant", "full") in {"full", "headless-inference"}:
            runtime = next(
                entry
                for entry in pins["targets"].values()
                if entry["target"] == targets[artifact["target"]]["rust_target"]
            )
            packages.append(
                {
                    "SPDXID": "SPDXRef-ORT",
                    "name": "onnxruntime-cpu",
                    "versionInfo": pins["version"],
                    "downloadLocation": runtime["url"],
                    "filesAnalyzed": False,
                    "licenseDeclared": "MIT",
                    "licenseConcluded": "NOASSERTION",
                    "copyrightText": "NOASSERTION",
                    "checksums": [
                        {"algorithm": "SHA256", "checksumValue": runtime["archive_sha256"]}
                    ],
                    "sourceInfo": "Reviewed native runtime input archive; package inspection and execution acceptance remain separate.",
                }
            )
            relationships.append(
                {
                    "spdxElementId": "SPDXRef-Pumas",
                    "relationshipType": "OTHER",
                    "relatedSpdxElement": "SPDXRef-ORT",
                    "comment": "Pinned runtime input for this artifact target.",
                }
            )
        for index, dependency in enumerate(inventories[profile]["packages"]):
            ident = f"SPDXRef-Dependency-{index}"
            packages.append(
                {
                    "SPDXID": ident,
                    "name": dependency["name"],
                    "versionInfo": dependency["version"],
                    "downloadLocation": dependency["source"],
                    "filesAnalyzed": False,
                    "licenseDeclared": "NOASSERTION",
                    "licenseConcluded": "NOASSERTION",
                    "copyrightText": "NOASSERTION",
                    "sourceInfo": dependency["scope"]
                    + "; declares "
                    + str(dependency["license_declared"]),
                }
            )
            relationships.append(
                {
                    "spdxElementId": "SPDXRef-Pumas",
                    "relationshipType": "OTHER",
                    "relatedSpdxElement": ident,
                    "comment": "Attribution input closure; includes build-only/other-target packages. Not runtime inclusion proof.",
                }
            )
        document = {
            "spdxVersion": "SPDX-2.3",
            "dataLicense": "CC0-1.0",
            "SPDXID": "SPDXRef-DOCUMENT",
            "name": filename,
            "documentNamespace": "https://pumas-library.local/spdx/" + sha,
            "creationInfo": {"created": created, "creators": ["Tool: pumas-release-metadata"]},
            "comment": "Artifact-level inventory bound to final bytes. Extracted files and native transitive components are not exhaustively analyzed. Unsigned candidate; maintainer inspection/license review remains required.",
            "packages": packages,
            "relationships": relationships,
        }
        with (directory / (filename + ".spdx.json")).open("x") as stream:
            stream.write(json.dumps(document, indent=2) + "\n")
    shutil.copyfile(repository / "LICENSE", directory / "LICENSE.txt")
    (directory / f"THIRD-PARTY-NOTICES-{version}.txt").write_bytes(b"\n\n".join(notices))
    provenance = {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": subjects,
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://github.com/MrScripty/Pumas-Library/candidate-assembly/v1",
                "externalParameters": {"build_id": build_id, "version": version},
                "internalParameters": {
                    "unsigned": True,
                    "qualification": "unverified_candidate",
                    "assurance": "self-reported assembly; no SLSA level or model/license acceptance claimed",
                    "source_tree": source["tree"],
                    "artifact_plan_sha256": digest(
                        repository / "scripts/release/artifact-plan.json"
                    ),
                    "rust_lock_sha256": digest(repository / "rust/Cargo.lock"),
                    "pnpm_lock_sha256": digest(repository / "pnpm-lock.yaml"),
                    "runtime_pins_sha256": digest(
                        repository / "scripts/release/onnx-runtime-pins.json"
                    ),
                    "attribution_input_profiles": {
                        profile or "default": {
                            "input_sha256": inventory["input_sha256"],
                            "notices_sha256": inventory["notices_sha256"],
                        }
                        for profile, inventory in inventories.items()
                    },
                },
                "resolvedDependencies": [
                    {
                        "uri": "git+https://github.com/MrScripty/Pumas-Library",
                        "digest": {"gitCommit": source["head"]},
                    }
                ],
            },
            "runDetails": {"builder": {"id": source["builder"]}},
        },
    }
    (directory / f"pumas-library-{version}.provenance.jsonl").write_text(
        json.dumps(provenance) + "\n"
    )
    final = sorted(p for p in directory.iterdir() if p.name != "checksums-sha256.txt")
    if any(p.is_symlink() or not p.is_file() for p in final):
        raise ValueError("final metadata inventory must contain regular files only")
    (directory / "checksums-sha256.txt").write_text(
        "".join(f"{digest(path)}  {path.name}\n" for path in final)
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build-id", required=True)
    args = parser.parse_args()

    def git(*arguments):
        return subprocess.check_output(["git", *arguments], cwd=ROOT, text=True).strip()

    if git("status", "--porcelain", "--untracked-files=no"):
        raise ValueError("candidate metadata requires unchanged tracked source inputs")
    source = {
        "head": git("rev-parse", "HEAD"),
        "tree": git("rev-parse", "HEAD^{tree}"),
        "builder": "https://github.com/"
        + os.environ.get("GITHUB_WORKFLOW_REF", "MrScripty/Pumas-Library/local-candidate-assembly"),
    }
    write_metadata(args.directory, args.build_id, source)


if __name__ == "__main__":
    main()
