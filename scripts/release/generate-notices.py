#!/usr/bin/env python3
"""Collect exact package license texts; fail rather than invent missing terms."""

import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
LICENSES = ROOT / "scripts/release/licenses"
OUTPUT = ROOT / "docs/release-attribution/0.7.0"


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def license_files(root):
    return sorted(
        p
        for p in root.rglob("*")
        if p.is_file()
        and (
            re.match(r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense)([._-].*)?$", p.name)
            or p.parent.name == "LICENSES"
        )
    )


def collect():
    metadata = json.loads(
        run(
            "cargo",
            "metadata",
            "--locked",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
            "--format-version",
            "1",
        )
    )
    tree = "\n".join(
        run(
            "cargo",
            "tree",
            "--locked",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "pumas-rpc",
            "--target",
            target,
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
        )
        for target in ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc")
    )
    selected = {tuple(line.split()[:2]) for line in tree.splitlines()}
    records, sections = [], []
    sources = json.loads((LICENSES / "sources.json").read_text())
    for source in sources:
        if digest((LICENSES / source["file"]).read_bytes()) != source["sha256"]:
            raise ValueError(f"Changed upstream license: {source['file']}")

    def add(name, version, license_id, source, root, files, scope):
        if not files:
            raise ValueError(f"No authoritative license files for {name}@{version}")
        record = {
            "name": name,
            "version": version,
            "license_declared": license_id,
            "source": source,
            "scope": scope,
            "texts": [],
        }
        section = [
            f"{name} {version} ({scope})",
            f"Source: {source}",
            f"Declared license: {license_id}",
        ]
        if license_id and "MPL-2.0" in license_id:
            section.append(
                "Unmodified covered source is available from the versioned source URL above."
            )
        for file in files:
            data = file.read_bytes()
            label = str(file.relative_to(root))
            section.extend([f"--- {label} ---", data.decode("utf-8")])
            record["texts"].append({"path": label, "sha256": digest(data)})
        records.append(record)
        sections.append("\n".join(section))

    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if (package["name"], "v" + package["version"]) not in selected or package["source"] is None:
            continue
        root = Path(package["manifest_path"]).parent
        files = license_files(root)
        if package["name"] in (
            "binrw",
            "binrw_derive",
            "governor",
            "block2",
            "dispatch2",
            "objc2",
            "objc2-encode",
        ):
            revision = json.loads((root / ".cargo_vcs_info.json").read_text())["git"]["sha1"]
            name = (
                "binrw-LICENSE"
                if package["name"].startswith("binrw")
                else package["name"] + "-LICENSE"
            )
            provenance = next(source for source in sources if source["file"] == name)
            if revision not in provenance["source"]:
                raise ValueError(f"Upstream license revision changed: {package['name']}")
            root = LICENSES
            files = [
                root
                / (
                    "binrw-LICENSE"
                    if package["name"].startswith("binrw")
                    else package["name"] + "-LICENSE"
                )
            ]
        add(
            package["name"],
            package["version"],
            package["license"],
            f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download",
            root,
            files,
            "Rust normal/build dependency, all desktop targets; conservative superset",
        )

    # Include the production JS closure, not just tree-shaken renderer modules.
    # Resolve from each package's own directory to follow pnpm's exact graph.
    js = json.loads(
        run(
            "node",
            "--input-type=module",
            "-e",
            """
import {createRequire} from 'node:module';
import fs from 'node:fs'; import path from 'node:path';
const seen=new Map();
function walk(dir) {
 const p=JSON.parse(fs.readFileSync(path.join(dir,'package.json')));
 for(const name of Object.keys(p.dependencies??{})) {
  const req=createRequire(path.join(dir,'package.json'));
  let manifest;
  try { manifest=req.resolve(name+'/package.json'); }
  catch { let d=path.dirname(req.resolve(name)); while(!fs.existsSync(path.join(d,'package.json'))) d=path.dirname(d); manifest=path.join(d,'package.json'); }
  const child=JSON.parse(fs.readFileSync(manifest));
  const key=child.name+'@'+child.version;
  if(seen.has(key))continue;
  const base=fs.realpathSync(path.dirname(manifest));
  seen.set(key,{...child,dir:base}); walk(base);
 }
}
walk(path.resolve('frontend'));walk(path.resolve('electron'));
console.log(JSON.stringify([...seen.values()]));
""",
        )
    )
    for package in sorted(js, key=lambda p: (p["name"], p["version"])):
        root = Path(package["dir"])
        files = [p for p in license_files(root) if "node_modules" not in p.relative_to(root).parts]
        add(
            package["name"],
            package["version"],
            package.get("license"),
            f"https://www.npmjs.com/package/{package['name']}/v/{package['version']}",
            root,
            files,
            "JavaScript production dependency closure; includes tree-shaken code",
        )
    add(
        "ONNX Runtime",
        "1.24.2",
        "MIT",
        "https://github.com/microsoft/onnxruntime/tree/v1.24.2",
        LICENSES,
        [
            LICENSES / "onnxruntime-LICENSE",
            LICENSES / "onnxruntime-ThirdPartyNotices.txt",
            LICENSES / "pyke-ort-artifacts-LICENSE",
        ],
        "Native runtime; upstream notice superset, downstream archive provenance tracked separately",
    )
    add(
        "uv",
        "0.12.18",
        "MIT OR Apache-2.0",
        "https://github.com/astral-sh/uv/tree/0.12.18",
        LICENSES,
        [
            LICENSES / "uv-0.12.18-LICENSE-MIT",
            LICENSES / "uv-0.12.18-LICENSE-APACHE",
        ],
        "Managed Python provider runtime executable downloaded by Pumas; not bundled",
    )
    sqlite = next(p for p in metadata["packages"] if p["name"] == "libsqlite3-sys")
    sqlite_source = Path(sqlite["manifest_path"]).parent / "sqlite3/sqlite3.c"
    blessing = sqlite_source.read_text().split("*/", 1)[0] + "*/"
    sections.append(
        "SQLite bundled by libsqlite3-sys 0.30.1\nSource: https://www.sqlite.org/\n" + blessing
    )
    electron = ROOT / "electron/node_modules/electron"
    version = json.loads((electron / "package.json").read_text())["version"]
    add(
        "Electron",
        version,
        "MIT",
        f"https://github.com/electron/electron/tree/v{version}",
        electron,
        [electron / "LICENSE"],
        "Desktop runtime; bundled Chromium notices remain adjacent to executable",
    )
    OUTPUT.mkdir(parents=True, exist_ok=True)
    header = """Pumas Library 0.7.0 — Third-party notices

Texts below are reproduced from the resolved packages or pinned upstream sources.
Rust normal/build dependencies across desktop targets and the JavaScript production
closure are conservative supersets, not assertions that all listed code is linked.
Electron also supplies LICENSE.electron.txt and LICENSES.chromium.html beside the
application executable. Those files are part of this attribution and must be retained.
ONNX upstream notices are included in full; downstream native closure verification
is recorded in the release attribution report, not inferred from this list.
The uv notice covers a runtime executable downloaded by Pumas, not a bundled binary.
Managed CPython archive licenses require separate archive-specific attribution.

"""
    text = header + ("\n\n" + "=" * 78 + "\n\n").join(sections) + "\n"
    (OUTPUT / "THIRD-PARTY-NOTICES.txt").write_text(text)
    inventory = {
        "schema_version": 1,
        "packages": records,
        "sources": sources,
        "input_sha256": {
            p: digest((ROOT / p).read_bytes())
            for p in (
                "rust/Cargo.lock",
                "rust/Cargo.toml",
                "rust/crates/pumas-core/Cargo.toml",
                "rust/crates/pumas-rpc/Cargo.toml",
                "rust/crates/pumas-app-manager/Cargo.toml",
                "pnpm-lock.yaml",
                "electron/package.json",
                "frontend/package.json",
            )
        },
        "notices_sha256": digest(text.encode()),
    }
    (OUTPUT / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    print(f"Collected {len(records)} package notice entries into {OUTPUT}")


if __name__ == "__main__":
    collect()
