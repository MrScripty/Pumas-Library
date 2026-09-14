#!/usr/bin/env python3
"""Write SPDX inventories and explicitly unsigned local build provenance."""

import argparse
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def hash_file(path, algorithm="sha256"):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, algorithm).hexdigest()


def write_metadata(directory, extracted):
    inventory = json.loads((ROOT / "docs/release-attribution/0.7.0/inventory.json").read_text())
    notices = ROOT / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt"
    for name, expected in inventory["input_sha256"].items():
        if hash_file(ROOT / name) != expected:
            raise ValueError(f"Attribution inputs changed: {name}")
    if hash_file(notices) != inventory["notices_sha256"]:
        raise ValueError("Attribution text changed")
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    subjects = []
    for filename, tree in (
        ("Pumas.Library-0.7.0.AppImage", extracted / "appimage/squashfs-root"),
        ("pumas-library-electron_0.7.0_amd64.deb", extracted / "deb/opt/Pumas Library"),
    ):
        artifact = directory / filename
        if hash_file(tree / "resources/THIRD-PARTY-NOTICES.txt") != hash_file(notices):
            raise ValueError(f"Installer attribution mismatch: {filename}")
        artifact_hash = hash_file(artifact)
        subjects.append({"name": filename, "digest": {"sha256": artifact_hash}})
        files = []
        for path in sorted(tree.rglob("*")):
            if not path.is_file() or path.is_symlink():
                continue
            files.append(
                {
                    "fileName": "./" + path.relative_to(tree).as_posix(),
                    "SPDXID": f"SPDXRef-File-{len(files)}",
                    "checksums": [
                        {"algorithm": "SHA1", "checksumValue": hash_file(path, "sha1")},
                        {"algorithm": "SHA256", "checksumValue": hash_file(path)},
                    ],
                    "licenseConcluded": "NOASSERTION",
                    "licenseInfoInFiles": ["NOASSERTION"],
                    "copyrightText": "NOASSERTION",
                }
            )
        package = {
            "name": "Pumas Library",
            "SPDXID": "SPDXRef-Pumas",
            "versionInfo": "0.7.0",
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": True,
            "packageVerificationCode": {
                "packageVerificationCodeValue": hashlib.sha1(
                    "".join(sorted(f["checksums"][0]["checksumValue"] for f in files)).encode()
                ).hexdigest()
            },
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "MIT",
            "licenseInfoFromFiles": ["NOASSERTION"],
            "copyrightText": "NOASSERTION",
            "checksums": [{"algorithm": "SHA256", "checksumValue": artifact_hash}],
            "sourceInfo": "Exact extracted application file inventory. Symlinks are excluded from file checksums.",
        }
        relationships = [
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": "SPDXRef-Pumas",
            }
        ]
        relationships += [
            {
                "spdxElementId": "SPDXRef-Pumas",
                "relationshipType": "CONTAINS",
                "relatedSpdxElement": f["SPDXID"],
            }
            for f in files
        ]
        packages = [package]
        for index, dep in enumerate(inventory["packages"]):
            ident = f"SPDXRef-Dependency-{index}"
            packages.append(
                {
                    "name": dep["name"],
                    "SPDXID": ident,
                    "versionInfo": dep["version"],
                    "downloadLocation": dep["source"],
                    "filesAnalyzed": False,
                    "licenseConcluded": "NOASSERTION",
                    "licenseDeclared": "NOASSERTION",
                    "copyrightText": "NOASSERTION",
                    "sourceInfo": dep["scope"]
                    + "; package declares "
                    + str(dep["license_declared"]),
                }
            )
            relationships.append(
                {
                    "spdxElementId": "SPDXRef-Pumas",
                    "relationshipType": "OTHER",
                    "relatedSpdxElement": ident,
                    "comment": "Attribution input closure; includes build-only and other-target packages, not proof of runtime inclusion.",
                }
            )
        document = {
            "spdxVersion": "SPDX-2.3",
            "dataLicense": "CC0-1.0",
            "SPDXID": "SPDXRef-DOCUMENT",
            "name": filename,
            "documentNamespace": "https://pumas-library.local/spdx/" + artifact_hash,
            "creationInfo": {"created": now, "creators": ["Tool: pumas-release-metadata"]},
            "comment": "Local candidate inventory. Native transitive components are retained in vendor notices; this is not an exhaustive component-level native SBOM or signed attestation.",
            "packages": packages,
            "files": files,
            "relationships": relationships,
        }
        (directory / (filename + ".spdx.json")).write_text(json.dumps(document, indent=2) + "\n")
    shutil.copyfile(notices, directory / "THIRD-PARTY-NOTICES-0.7.0.txt")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    patch = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=ROOT)
    provenance = {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": subjects,
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://github.com/MrScripty/Pumas-Library/local-release-build/v1",
                "externalParameters": {
                    "command": "electron-builder --linux --publish never",
                    "platform": "linux-x86_64",
                },
                "internalParameters": {
                    "unsigned": True,
                    "source_revision_role": "repository revision observed at metadata generation; material input hashes identify the packaging inputs",
                    "assurance": "local self-reported candidate; no SLSA level claimed",
                    "working_tree_patch_sha256": hashlib.sha256(patch).hexdigest(),
                    "input_sha256": inventory["input_sha256"],
                    "notices_sha256": inventory["notices_sha256"],
                },
                "resolvedDependencies": [
                    {
                        "uri": "git+https://github.com/MrScripty/Pumas-Library",
                        "digest": {"gitCommit": revision},
                    }
                ],
            },
            "runDetails": {
                "builder": {
                    "id": "https://github.com/MrScripty/Pumas-Library/local-release-preparation"
                }
            },
        },
    }
    (directory / "pumas-library-0.7.0.provenance.jsonl").write_text(json.dumps(provenance) + "\n")
    final = sorted(
        p for p in directory.iterdir() if p.is_file() and p.name != "checksums-sha256.txt"
    )
    (directory / "checksums-sha256.txt").write_text(
        "".join(f"{hash_file(p)}  {p.name}\n" for p in final)
    )
    print(
        f"Wrote per-installer SPDX inventories, unsigned provenance and checksums for {len(final)} files"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("extracted", type=Path)
    args = parser.parse_args()
    write_metadata(args.directory, args.extracted)
