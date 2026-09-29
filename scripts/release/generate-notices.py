#!/usr/bin/env python3
"""Collect exact package license texts; fail rather than invent missing terms."""

import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[2]
LICENSES = ROOT / "scripts/release/licenses"
OUTPUT = ROOT / "docs/release-attribution/0.7.0"
MANAGED_PYTHON_CATALOG = "scripts/release/licenses/managed-python-sources.json"
ARTIFACT_PLAN = "scripts/release/artifact-plan.json"
MANAGED_PYTHON_PINS = "rust/crates/pumas-app-manager/src/version_manager/managed_python.rs"
PBS_MAPPING = "scripts/release/torch-managed-python-acceptance.py"
UV_ARM_TARGETS = {
    "LinuxX8664": "x86_64-unknown-linux-gnu",
    "WindowsX8664": "x86_64-pc-windows-msvc",
    "MacosArm64": "aarch64-apple-darwin",
}


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


def repository_file(name):
    if (
        not isinstance(name, str)
        or not name
        or Path(name).is_absolute()
        or "\\" in name
        or any(part in ("", ".", "..") for part in name.split("/"))
    ):
        raise ValueError(f"Invalid attribution path: {name}")
    path = ROOT / name
    if ".." in Path(name).parts or not path.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError(f"Attribution path escapes repository: {name}")
    return path


def retained_provider_pins(expected_targets):
    source = repository_file(MANAGED_PYTHON_PINS).read_text(encoding="utf-8")
    version = re.findall(r'^const UV_VERSION: &str = "([0-9]+\.[0-9]+\.[0-9]+)";', source, re.M)
    base = re.findall(
        r'^const UV_BASE_URL: &str = "https://releases\.astral\.sh/github/uv/releases/download/([0-9.]+)/";',
        source,
        re.M,
    )
    block = re.search(r"fn pin\(self\) -> UvPin \{(.*?)\n    fn accepts_observed", source, re.S)
    pins = re.findall(
        r'Self::(\w+) => UvPin \{\s*archive: "uv-([a-z0-9_-]+)\.(?:tar\.gz|zip)",\s*sha256: "([0-9a-f]{64})"',
        block.group(1) if block else "",
    )
    if (
        len(version) != 1
        or base != version
        or len(pins) != 3
        or {arm for arm, _, _ in pins} != set(UV_ARM_TARGETS)
        or {target for _, target, _ in pins} != expected_targets
        or any(UV_ARM_TARGETS[arm] != target for arm, target, _ in pins)
    ):
        raise ValueError("Managed uv pins do not match desktop targets")
    return version[0], {target: sha256 for _, target, sha256 in pins}


def retained_full_archive_mapping(expected_targets):
    source = repository_file(PBS_MAPPING).read_text(encoding="utf-8")
    release = re.findall(r'^REVIEWED_PBS_RELEASE = "([0-9]{8})"$', source, re.M)
    block = re.search(r"^FULL_FLAVORS = \{(.*?)^\}", source, re.M | re.S)
    flavors = re.findall(
        r'^    "([a-z0-9_-]+)": "([a-z0-9+]+)",$', block.group(1) if block else "", re.M
    )
    if (
        len(release) != 1
        or len(flavors) != 3
        or {target for target, _ in flavors} != expected_targets
    ):
        raise ValueError("Managed CPython full-archive mapping does not match desktop targets")
    return release[0], dict(flavors)


def collect_managed_python(add):
    plan = json.loads(repository_file(ARTIFACT_PLAN).read_bytes())
    expected_targets = {item["rust_target"] for item in plan["desktop_targets"]}
    uv_version, uv_pins = retained_provider_pins(expected_targets)
    pbs_release, full_flavors = retained_full_archive_mapping(expected_targets)
    catalog = json.loads(repository_file(MANAGED_PYTHON_CATALOG).read_bytes())
    entries = catalog.get("targets")
    if catalog.get("schema_version") != 1 or not isinstance(entries, list):
        raise ValueError("Invalid managed CPython attribution catalog")
    if (
        len(expected_targets) != 3
        or len(entries) != 3
        or {item.get("target") for item in entries} != expected_targets
    ):
        raise ValueError("Managed CPython attribution must cover exactly three native targets")
    inventory, inputs = (
        [],
        {ARTIFACT_PLAN, MANAGED_PYTHON_CATALOG, MANAGED_PYTHON_PINS, PBS_MAPPING},
    )
    for item in sorted(entries, key=lambda item: item["target"]):
        target = item["target"]
        report_name = item["runtime_report"]
        manifest_name = item["manifest"]
        report = json.loads(repository_file(report_name).read_bytes())
        manifest_path = repository_file(manifest_name)
        manifest = json.loads(manifest_path.read_bytes())
        inputs.update((report_name, manifest_name))
        distribution = report["managed_python"]["distribution"]
        provider = report["managed_python"]["provider"]
        version = distribution["version"]
        source_url = distribution["sourceUrl"]
        full_name = f"cpython-{version}+{pbs_release}-{target}-{full_flavors[target]}-full.tar.zst"
        expected_full_url = (
            "https://github.com/astral-sh/python-build-standalone/releases/download/"
            f"{pbs_release}/{quote(full_name, safe='-._~')}"
        )
        expected_selected_urls = {
            "https://releases.astral.sh/github/python-build-standalone/releases/download/"
            f"{pbs_release}/{quote(f'cpython-{version}+{pbs_release}-{target}-{flavor}.tar.gz', safe='-._~')}"
            for flavor in ("install_only", "install_only_stripped")
        }
        if (
            distribution["implementation"] != "CPython"
            or distribution["targetTriple"] != target
            or not re.fullmatch(r"\d+\.\d+\.\d+", version)
            or not distribution["catalogKey"].startswith(f"cpython-{version}-")
            or provider["name"] != "uv"
            or provider["version"] != uv_version
            or provider["archiveSha256"] != uv_pins[target]
            or manifest["target"] != target
            or manifest["source_url"] != source_url
            or source_url not in expected_selected_urls
            or manifest["archive_url"] != expected_full_url
            or manifest["archive_name"] != full_name
            or not re.fullmatch(r"[0-9a-f]{64}", manifest["archive_sha256"])
        ):
            raise ValueError(f"Managed CPython selected/full archive mismatch: {target}")
        full_archive = manifest_path.parent / "full-archive"
        python_ref = manifest["python_json"]
        if python_ref["path"] != "PYTHON.json":
            raise ValueError(f"Invalid managed CPython PYTHON.json path: {target}")
        python_name = (full_archive / "PYTHON.json").relative_to(ROOT).as_posix()
        python_bytes = repository_file(python_name).read_bytes()
        if digest(python_bytes) != python_ref["sha256"] or len(python_bytes) != python_ref["size"]:
            raise ValueError(f"Changed managed CPython PYTHON.json: {target}")
        python = json.loads(python_bytes)
        if (
            python["target_triple"] != target
            or python["python_version"] != version
            or python["build_options"] != full_flavors[target]
        ):
            raise ValueError(f"Managed CPython PYTHON.json identity mismatch: {target}")
        inputs.add(python_name)
        files = manifest["files"]
        if not isinstance(files, list) or not files:
            raise ValueError(f"No managed CPython legal files: {target}")
        names = [entry["path"] for entry in files]
        if len(names) != len(set(names)) or any(
            not re.fullmatch(r"licenses/[A-Za-z0-9._-]+", name) for name in names
        ):
            raise ValueError(f"Invalid managed CPython legal file list: {target}")
        if (
            set(names)
            != {
                path.relative_to(full_archive).as_posix()
                for path in (full_archive / "licenses").iterdir()
                if path.is_file()
            }
            or python["license_path"] not in names
        ):
            raise ValueError(f"Incomplete managed CPython legal file list: {target}")
        legal_files = []
        for entry in sorted(files, key=lambda entry: entry["path"]):
            name = (full_archive / entry["path"]).relative_to(ROOT).as_posix()
            data = repository_file(name).read_bytes()
            if digest(data) != entry["sha256"]:
                raise ValueError(f"Changed managed CPython legal file: {name}")
            legal_files.append({"path": name, "sha256": entry["sha256"]})
            inputs.add(name)
        add(
            "CPython full archive",
            version,
            ", ".join(python["licenses"]),
            manifest["archive_url"],
            full_archive,
            [
                full_archive / entry["path"]
                for entry in sorted(files, key=lambda entry: entry["path"])
            ],
            f"Managed Python full-archive notice superset for {target}; selected install-only runtime tracked separately",
        )
        inventory.append(
            {
                "target": target,
                "runtime_report": report_name,
                "manifest": manifest_name,
                "catalog_key": distribution["catalogKey"],
                "selected_install_only_url": source_url,
                "provider_name": provider["name"],
                "provider_version": provider["version"],
                "provider_archive_sha256": provider["archiveSha256"],
                "full_archive_url": manifest["archive_url"],
                "full_archive_sha256": manifest["archive_sha256"],
                "python_json": python_name,
                "legal_files": legal_files,
            }
        )
    return inventory, inputs, uv_version


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
    managed_python, managed_inputs, uv_version = collect_managed_python(add)
    add(
        "uv",
        uv_version,
        "MIT OR Apache-2.0",
        f"https://github.com/astral-sh/uv/tree/{uv_version}",
        LICENSES,
        [
            LICENSES / f"uv-{uv_version}-LICENSE-MIT",
            LICENSES / f"uv-{uv_version}-LICENSE-APACHE",
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
Managed CPython full-archive notices are target-scoped supersets for separately
selected install-only distributions; each archive identity is recorded below.

"""
    text = header + ("\n\n" + "=" * 78 + "\n\n").join(sections) + "\n"
    (OUTPUT / "THIRD-PARTY-NOTICES.txt").write_bytes(text.encode("utf-8"))
    inventory = {
        "schema_version": 1,
        "packages": records,
        "sources": sources,
        "managed_python": managed_python,
        "input_sha256": {
            p: digest((ROOT / p).read_bytes())
            for p in sorted(
                {
                    "rust/Cargo.lock",
                    "rust/Cargo.toml",
                    "rust/crates/pumas-core/Cargo.toml",
                    "rust/crates/pumas-rpc/Cargo.toml",
                    "rust/crates/pumas-app-manager/Cargo.toml",
                    "pnpm-lock.yaml",
                    "electron/package.json",
                    "frontend/package.json",
                    *managed_inputs,
                }
            )
        },
        "notices_sha256": digest(text.encode()),
    }
    (OUTPUT / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    print(f"Collected {len(records)} package notice entries into {OUTPUT}")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--output":
        OUTPUT = Path(sys.argv[2]).resolve()
    elif len(sys.argv) != 1:
        raise SystemExit("usage: generate-notices.py [--output DIRECTORY]")
    collect()
