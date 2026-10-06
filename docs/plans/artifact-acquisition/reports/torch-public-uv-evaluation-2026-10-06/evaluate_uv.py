"""Evidence-only public CLI controls. Serve metadata-only wheels, never source.

Source routes refuse bodies unless they contain our data-only archive. A trusted git sentinel records invocation and
exits before fetching. No fixture contains a backend, setup.py or importable code.
"""

import argparse
import base64
import csv
from functools import partial
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import os
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tarfile
import threading
import time
import tomllib
from urllib.parse import urlparse
import zipfile


def wheel(name, version="1.0", *, requires=(), extras=(), tag="py3-none-any", python=None):
    normalized = name.replace("-", "_")
    info = f"{normalized}-{version}.dist-info"
    metadata = f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n"
    metadata += "".join(f"Requires-Dist: {value}\n" for value in requires)
    metadata += "".join(f"Provides-Extra: {value}\n" for value in extras)
    if python:
        metadata += f"Requires-Python: {python}\n"
    contents = {
        info + "/METADATA": (metadata + "\n").encode(),
        info + "/WHEEL": f"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: {tag}\n".encode(),
    }
    record = io.StringIO()
    writer = csv.writer(record, lineterminator="\n")
    for member, body in contents.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(body).digest()).rstrip(b"=").decode()
        writer.writerow([member, "sha256=" + digest, len(body)])
    writer.writerow([info + "/RECORD", "", ""])
    contents[info + "/RECORD"] = record.getvalue().encode()
    result = io.BytesIO()
    with zipfile.ZipFile(result, "w") as archive:
        for member, body in contents.items():
            entry = zipfile.ZipInfo(member, date_time=(2020, 1, 1, 0, 0, 0))
            entry.external_attr = 0o644 << 16
            archive.writestr(entry, body)
    return f"{normalized}-{version}-{tag}.whl", result.getvalue()


class Fixture:
    def __init__(self):
        self.responses = {}
        self.requests = []
        self.projects = {}
        self.inventory = []
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Handler, fixture=self))
        self.base = f"http://127.0.0.1:{self.server.server_port}"
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.start()

    def add(self, name, version="1.0", *, index="one", **kwargs):
        filename, body = wheel(name, version, **kwargs)
        path = f"/files/{index}/{filename}"
        self.responses[path] = (200, "application/octet-stream", body)
        digest = hashlib.sha256(body).hexdigest()
        self.projects.setdefault((index, name), []).append((path, digest, kwargs.get("python")))
        self.inventory.append(
            {
                "name": name,
                "version": version,
                "filename": filename,
                "sha256": digest,
                "bytes": len(body),
                "index": index,
                "metadata": kwargs,
            }
        )
        self.refresh(index, name)
        return self.base + path, digest

    def refresh(self, index, name):
        links = []
        for path, digest, python in self.projects[index, name]:
            # Synthetic Requires-Python values are controlled ASCII specifiers.
            attribute = f' data-requires-python="{python.replace(">", "&gt;")}"' if python else ""
            links.append(
                f'<a href="{path}#sha256={digest}"{attribute}>{path.rsplit("/", 1)[1]}</a>'
            )
        self.responses[f"/{index}/simple/{name}/"] = (200, "text/html", "\n".join(links).encode())

    def data_source(self):
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode="w:gz") as archive:
            entries = {
                "source-1.0/PKG-INFO": b"Metadata-Version: 2.2\nName: source\nVersion: 1.0\nDynamic: Requires-Dist\n\n",
                "source-1.0/pyproject.toml": b'[build-system]\nrequires=["build-trap==99"]\nbuild-backend="pumas_missing_backend"\n[project]\nname="source"\nversion="1.0"\ndynamic=["dependencies"]\n',
            }
            for name, body in entries.items():
                item = tarfile.TarInfo(name)
                item.size = len(body)
                archive.addfile(item, io.BytesIO(body))
        self.responses["/trap/source-1.0.tar.gz"] = (
            200,
            "application/octet-stream",
            data.getvalue(),
        )
        return self.base + "/trap/source-1.0.tar.gz"

    def close(self):
        self.server.shutdown()
        self.thread.join()
        self.server.server_close()


class Handler(BaseHTTPRequestHandler):
    def __init__(self, *args, fixture, **kwargs):
        self.fixture = fixture
        super().__init__(*args, **kwargs)

    def log_message(self, *_args):
        pass

    def do_HEAD(self):
        self.respond(head=True)

    def do_GET(self):
        self.respond(head=False)

    def respond(self, head):
        path = urlparse(self.path).path
        self.fixture.requests.append(
            {
                "method": self.command,
                "path": path,
                "range": self.headers.get("Range"),
                "authorization_present": "Authorization" in self.headers,
            }
        )
        # Fail closed, even if the resolver fails a source-refusal control.
        status, content_type, body = self.fixture.responses.get(
            path, (503 if path.startswith("/trap/") else 404, "text/plain", b"")
        )
        full_length = len(body)
        content_range = None
        if status == 200 and path.startswith("/files/") and self.headers.get("Range"):
            match = re.fullmatch(r"bytes=(\d*)-(\d*)", self.headers["Range"])
            if match:
                first, last = match.groups()
                start = int(first) if first else max(0, full_length - int(last))
                end = min(int(last), full_length - 1) if first and last else full_length - 1
                if 0 <= start <= end < full_length:
                    status = 206
                    body = body[start : end + 1]
                    content_range = f"bytes {start}-{end}/{full_length}"
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Accept-Ranges", "bytes")
        if content_range:
            self.send_header("Content-Range", content_range)
        self.end_headers()
        if not head:
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                # A resolver may close an unused candidate response early.
                # The request remains in the evidence; no oracle is relaxed.
                pass


def environment(directory):
    # Use a deliberately small child environment, never host credentials/config.
    result = {
        key: os.environ[key]
        for key in ("PATH", "LANG", "LC_ALL", "SYSTEMROOT")
        if key in os.environ
    }
    result.update(
        {
            "NETRC": os.devnull,
            "UV_CREDENTIALS_DIR": str(directory / "credentials"),
            "XDG_DATA_HOME": str(directory / "data"),
            "XDG_CONFIG_HOME": str(directory / "config"),
            "XDG_CACHE_HOME": str(directory / "cache"),
            "UV_PYTHON_DOWNLOADS": "never",
            "UV_HTTP_RETRIES": "0",
            "PIP_CONFIG_FILE": os.devnull,
            "NO_PROXY": "127.0.0.1,localhost",
            "PUMAS_GIT_TRAP": str(directory / "git-attempts.txt"),
        }
    )
    trap = directory / "traps"
    trap.mkdir(exist_ok=True)
    sentinel = trap / "git"
    sentinel.write_text(
        "#!" + sys.executable + "\nimport os\nfrom pathlib import Path\n"
        "Path(os.environ['PUMAS_GIT_TRAP']).write_text('git invoked; no fetch performed\\n')\n"
        "raise SystemExit(98)\n"
    )
    sentinel.chmod(0o755)
    result["PATH"] = str(trap) + os.pathsep + result.get("PATH", "")
    return result


def lock_versions(document):
    return {package["name"]: package.get("version") for package in document.get("packages", [])}


class Evaluation:
    def __init__(self, binary, output):
        self.binary, self.output = binary, output
        output.mkdir(parents=True, exist_ok=True)
        self.records = []
        self.failures = []

    def run(
        self,
        case,
        requirements,
        *,
        setup=None,
        extra=(),
        target="3.12",
        platform="x86_64-unknown-linux-gnu",
        format="pylock.toml",
        pip=False,
        seed=None,
        real_git_env=None,
    ):
        fixture = Fixture()
        try:
            if setup:
                setup(fixture)
            text = requirements(fixture) if callable(requirements) else requirements
            with tempfile.TemporaryDirectory(prefix="pumas-uv-control-") as temporary:
                directory = Path(temporary)
                source = directory / "requirements.in"
                source.write_text(text + "\n")
                result_file = directory / (
                    "report.json"
                    if pip
                    else "pylock.toml"
                    if format == "pylock.toml"
                    else "requirements.txt"
                )
                if seed:
                    result_file.write_text(seed)
                child_env = environment(directory)
                if real_git_env:
                    child_env.update(real_git_env)
                (directory / "uv.toml").write_text(
                    'index-url="http://127.0.0.1:1/hostile-config/"\nresolution="lowest"\n'
                )
                if pip:
                    command = [
                        sys.executable,
                        "-I",
                        "-m",
                        "pip",
                        "--isolated",
                        "install",
                        "--dry-run",
                        "--ignore-installed",
                        "--no-cache-dir",
                        "--disable-pip-version-check",
                        "--only-binary=:all:",
                        "--index-url",
                        fixture.base + "/one/simple/",
                        "--report",
                        str(result_file),
                        "-r",
                        str(source),
                        *extra,
                    ]
                else:
                    command = [
                        str(self.binary),
                        "--no-config",
                        "--no-cache",
                        "--no-python-downloads",
                        "--no-managed-python",
                        "--color",
                        "never",
                        "pip",
                        "compile",
                        str(source),
                        "--only-binary",
                        ":all:",
                        "--no-sources",
                        "--keyring-provider",
                        "disabled",
                        "--python",
                        sys.executable,
                        "--python-version",
                        target,
                        "--default-index",
                        fixture.base + "/one/simple/",
                        "--format",
                        format,
                        "--generate-hashes",
                        "--no-header",
                        "--no-annotate",
                        "--output-file",
                        str(result_file),
                    ]
                    if platform:
                        command += ["--python-platform", platform]
                    command += list(extra)
                start = time.monotonic()
                completed = subprocess.run(
                    command,
                    capture_output=True,
                    text=True,
                    cwd=directory,
                    env=child_env,
                    timeout=20,
                )
                body = result_file.read_text() if result_file.exists() else None
                document = (
                    json.loads(body)
                    if pip and body
                    else tomllib.loads(body)
                    if format == "pylock.toml" and body
                    else None
                )
                versions = (
                    {
                        item["metadata"]["name"]: item["metadata"]["version"]
                        for item in document["install"]
                    }
                    if pip and document
                    else lock_versions(document)
                    if document
                    else {}
                )
                record = {
                    "case": case,
                    "command": command,
                    "requirements": text,
                    "exit": completed.returncode,
                    "elapsed_seconds": round(time.monotonic() - start, 4),
                    "stdout": completed.stdout,
                    "stderr": completed.stderr,
                    "output": body,
                    "document": document,
                    "versions": versions,
                    "requests": fixture.requests,
                    "git_invoked": (directory / "git-attempts.txt").exists(),
                    "fixture_inventory": fixture.inventory,
                    "installed_fixture_distributions": list(directory.rglob("*.dist-info")),
                    "cache_files": [
                        str(p.relative_to(directory))
                        for p in directory.rglob("*")
                        if p.is_file() and p.name not in {source.name, result_file.name, "git"}
                    ],
                    "source_code_or_backends_served": False,
                }
                assert not record["installed_fixture_distributions"]
                assert not any(row["authorization_present"] for row in fixture.requests)
                self.records.append(record)
                (self.output / (case + ".json")).write_text(json.dumps(record, indent=2) + "\n")
                print(
                    f"{case}: exit={record['exit']} versions={versions} requests={len(fixture.requests)} git={record['git_invoked']}",
                    flush=True,
                )
                return record
        finally:
            fixture.close()

    def check(self, record, condition, description):
        if not condition:
            self.failures.append({"case": record["case"], "condition": description})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uv", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    evaluation = Evaluation(args.uv, args.output)
    run, check = evaluation.run, evaluation.check

    def closure(f):
        f.add("root", requires=["middle[fast]==1.0", "inactive==1.0; python_version < '3.11'"])
        f.add("middle", requires=["leaf==1.0; extra == 'fast'"], extras=["fast"])
        f.add("leaf")

    valid = run("valid-closure-extras", "root==1.0", setup=closure)
    check(
        valid,
        valid["exit"] == 0 and valid["versions"] == {"root": "1.0", "middle": "1.0", "leaf": "1.0"},
        "complete extras closure",
    )
    check(
        valid,
        valid["document"].get("lock-version") == "1.0"
        or valid["document"].get("pylock-version") == "1.0",
        "versioned public lock schema",
    )

    for kind in ("sdist", "vcs"):

        def address(f, k=kind):
            return (
                f.base + "/trap/source-1.0.tar.gz"
                if k == "sdist"
                else "git+" + f.base + "/trap/repository"
            )

        record = run("direct-" + kind, lambda f: "source @ " + address(f))
        check(
            record,
            record["exit"] != 0 and record["output"] is None,
            "source root rejected without lock",
        )
        # Measurement: native compile can retrieve/init before refusal. This is
        # not a before-retrieval guarantee; data-only controls below test builds.
        check(record, record["exit"] != 0, "native source trap cannot yield an accepted lock")

        def dependency(f, reference=address):
            f.add("root", requires=["source @ " + reference(f)])

        record = run("transitive-" + kind, "root==1.0", setup=dependency)
        check(
            record,
            record["exit"] != 0 and record["output"] is None,
            "active source dependency refused",
        )
        check(
            record,
            not record["git_invoked"]
            and not any(r["path"].startswith("/trap/") for r in record["requests"]),
            "no source/VCS access",
        )

    def source_archive(f):
        source = f.data_source()
        url, _ = f.add("root", requires=["source @ " + source])
        f.direct = "root @ " + url

    for name, request in [
        ("data-only-sdist", lambda f: "source @ " + f.base + "/trap/source-1.0.tar.gz"),
        ("direct-wheel-transitive-sdist", lambda f: f.direct),
    ]:
        record = run(name, request, setup=source_archive)
        check(
            record,
            record["exit"] != 0
            and record["output"] is None
            and "Building source distributions" in record["stderr"],
            "dynamic sdist refused by binary build boundary",
        )
        check(
            record,
            not any("build-trap" in r["path"] for r in record["requests"]),
            "no build dependency request or install",
        )

    with tempfile.TemporaryDirectory(prefix="pumas-inert-git-") as temporary:
        repo = Path(temporary) / "repo"
        repo.mkdir()
        hooks = Path(temporary) / "empty-hooks"
        hooks.mkdir()
        (repo / "pyproject.toml").write_text(
            '[build-system]\nrequires=["build-trap==99"]\nbuild-backend="pumas_missing_backend"\n[project]\nname="source"\nversion="1.0"\ndynamic=["dependencies"]\n'
        )
        git_env = {
            "PATH": "/usr/bin:/bin",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_ALLOW_PROTOCOL": "file",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "core.hooksPath",
            "GIT_CONFIG_VALUE_0": str(hooks),
            "GIT_TERMINAL_PROMPT": "0",
        }
        for command in [
            ["init", "--quiet", str(repo)],
            ["-C", str(repo), "add", "pyproject.toml"],
            [
                "-C",
                str(repo),
                "-c",
                "user.name=Inert Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "metadata only",
            ],
        ]:
            subprocess.run(["/usr/bin/git", *command], env=git_env, check=True)
        commit = subprocess.check_output(
            ["/usr/bin/git", "-C", str(repo), "rev-parse", "HEAD"], env=git_env, text=True
        ).strip()
        reference = "git+" + repo.as_uri() + "@" + commit

        def vcs_root(f):
            url, _ = f.add("root", requires=["source @ " + reference])
            f.direct = "root @ " + url

        for name, request in [
            ("data-only-local-vcs", lambda f: "source @ " + reference),
            ("direct-wheel-transitive-local-vcs", lambda f: f.direct),
        ]:
            record = run(name, request, setup=vcs_root, real_git_env=git_env)
            check(
                record,
                record["exit"] != 0
                and record["output"] is None
                and "Building source distributions" in record["stderr"],
                "local metadata-only VCS refused before build",
            )
            check(
                record,
                not any("build-trap" in r["path"] for r in record["requests"]),
                "VCS build dependency never requested",
            )

    def inactive(f):
        f.add(
            "root", requires=[f"source @ {f.base}/trap/source-1.0.tar.gz ; python_version < '3.0'"]
        )

    record = run("inactive-source-marker", "root==1.0", setup=inactive)
    check(
        record,
        record["exit"] == 0 and not any(r["path"].startswith("/trap/") for r in record["requests"]),
        "inactive URL branch not retrieved",
    )

    def python_case(f):
        f.add("root", requires=["leaf==1.0; python_version >= '3.13'"])
        f.add("leaf", python=">=3.13")

    for target in ("3.12", "3.13"):
        record = run("target-python-" + target, "root==1.0", setup=python_case, target=target)
        expected = {"root": "1.0", **({"leaf": "1.0"} if target == "3.13" else {})}
        check(
            record,
            record["exit"] == 0 and record["versions"] == expected,
            "target Python marker closure",
        )
    for target in ("3.12", "3.13"):
        record = run(
            "requires-python-" + target,
            "leaf==1.0",
            setup=lambda f: f.add("leaf", python=">=3.13"),
            target=target,
        )
        check(
            record, (record["exit"] == 0) == (target == "3.13"), "target Requires-Python enforced"
        )

    def tags(f):
        f.add("tagged", "1.0", tag="cp312-cp312-manylinux_2_17_x86_64")
        f.add("tagged", "2.0", tag="cp313-cp313-manylinux_2_17_x86_64")
        f.add("tagged", "3.0", tag="cp312-cp312-win_amd64")

    for target, platform, expected in [
        ("3.12", "x86_64-unknown-linux-gnu", "1.0"),
        ("3.13", "x86_64-unknown-linux-gnu", "2.0"),
        ("3.12", "x86_64-pc-windows-msvc", "3.0"),
    ]:
        record = run(
            "tags-" + target + "-" + platform,
            "tagged",
            setup=tags,
            target=target,
            platform=platform,
        )
        check(
            record,
            record["exit"] == 0 and record["versions"].get("tagged") == expected,
            "native wheel tag selection",
        )

    def recent_linux(f):
        f.add("tagged", tag="cp312-cp312-manylinux_2_28_x86_64")

    for platform in ("x86_64-unknown-linux-gnu", "x86_64-manylinux_2_28", None):
        record = run(
            "glibc-" + (platform or "native"), "tagged==1.0", setup=recent_linux, platform=platform
        )
        check(
            record,
            record["exit"] == 0,
            "current uv target/native accept this manylinux2.28 fixture",
        )

    for platform in ("x86_64-manylinux_2_17", "x86_64-manylinux_2_28"):
        record = run(
            "explicit-glibc-" + platform, "tagged==1.0", setup=recent_linux, platform=platform
        )
        check(
            record,
            (record["exit"] == 0) == (platform == "x86_64-manylinux_2_28"),
            "explicit manylinux version determines wheel admission",
        )

    def inventory(f):
        f.add("catalogue", tag="cp312-cp312-manylinux_2_17_x86_64")
        f.add("catalogue", tag="cp312-cp312-win_amd64")

    record = run("lock-artifact-inventory", "catalogue==1.0", setup=inventory)
    check(record, record["exit"] == 0, "versioned lock enumerates package artifact inventory")

    def compatible_inventory(f):
        f.add("catalogue", tag="py3-none-any")
        f.add("catalogue", tag="cp312-cp312-manylinux_2_17_x86_64")

    record = run("lock-multiple-compatible-artifacts", "catalogue==1.0", setup=compatible_inventory)
    check(
        record,
        record["exit"] == 0,
        "compatible artifact inventory is measured rather than assumed to be a pip selected-file report",
    )

    def platform_markers(f):
        f.add(
            "root",
            requires=[
                "linux-leaf==1.0; sys_platform == 'linux'",
                "windows-leaf==1.0; sys_platform == 'win32'",
            ],
        )
        f.add("linux-leaf")
        f.add("windows-leaf")

    for platform, leaf in [
        ("x86_64-unknown-linux-gnu", "linux-leaf"),
        ("x86_64-pc-windows-msvc", "windows-leaf"),
    ]:
        record = run(
            "platform-markers-" + platform, "root==1.0", setup=platform_markers, platform=platform
        )
        check(
            record,
            record["exit"] == 0 and record["versions"] == {"root": "1.0", leaf: "1.0"},
            "target OS markers use requested platform",
        )

    def direct_root(f):
        url, digest = f.add("root", requires=["leaf==1.0"])
        f.add("leaf")
        f.direct = "root @ " + url + "#sha256=" + digest

    record = run("direct-wheel-root", lambda f: f.direct, setup=direct_root)
    check(
        record,
        record["exit"] == 0 and record["versions"] == {"root": "1.0", "leaf": "1.0"},
        "exact direct wheel root closure",
    )
    wrong = run(
        "wrong-direct-root-hash",
        lambda f: f.direct.rsplit("=", 1)[0] + "=" + "0" * 64,
        setup=direct_root,
    )
    check(
        wrong,
        wrong["exit"] == 0
        and next(p for p in wrong["document"]["packages"] if p["name"] == "root")["archive"][
            "hashes"
        ]["sha256"]
        == "0" * 64,
        "compile retains but does not verify supplied hash",
    )

    owner_path = Path(__file__).resolve().parents[5] / "torch-server" / "install_verified_wheels.py"
    spec = importlib.util.spec_from_file_location("existing_local_owner", owner_path)
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    with tempfile.TemporaryDirectory(prefix="pumas-hash-preflight-") as temporary:
        artifacts = []
        for package in wrong["document"]["packages"]:
            entry = package.get("archive") or package["wheels"][0]
            url = entry["url"]
            item = next(i for i in wrong["fixture_inventory"] if i["name"] == package["name"])
            filename, body = wheel(item["name"], item["version"], **item["metadata"])
            (Path(temporary) / filename).write_bytes(body)
            artifacts.append(
                {
                    "name": package["name"],
                    "version": package["version"],
                    "url": url,
                    "sha256": entry["hashes"]["sha256"],
                }
            )
        refused = False
        try:
            owner.local_requirements(artifacts, Path(temporary))
        except ValueError:
            refused = True
        check(wrong, refused, "actual acquired wheel preflight refuses the claimed wrong hash")

    with tempfile.TemporaryDirectory(prefix="pumas-uv-local-proof-") as temporary:
        root = Path(temporary)
        wheels = root / "wheels"
        wheels.mkdir()
        artifacts = []
        for package in valid["document"]["packages"]:
            entry = package["wheels"][0]
            item = next(i for i in valid["fixture_inventory"] if i["name"] == package["name"])
            filename, body = wheel(item["name"], item["version"], **item["metadata"])
            (wheels / filename).write_bytes(body)
            artifacts.append(
                {
                    "name": package["name"],
                    "version": package["version"],
                    "url": entry["url"],
                    "sha256": entry["hashes"]["sha256"],
                }
            )
        manifest = owner.install(artifacts, wheels, root / "target", root / "proof")
        check(
            valid,
            len(manifest["files"]) > 0,
            "valid uv lock feeds actual hash/closure preflight and local pip/RECORD proof",
        )
        (args.output / "local-install-proof.json").write_text(json.dumps(manifest, indent=2) + "\n")

    def indexes(f):
        f.add("choice", "1.0", index="one")
        f.add("choice", "2.0", index="two")

    for strategy in (None, "first-index", "unsafe-first-match", "unsafe-best-match"):
        # Expose the extra index through an input option, not an external URL.
        record = run(
            "index-" + (strategy or "default"),
            lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
            setup=indexes,
            extra=() if strategy is None else ("--index-strategy", strategy),
        )
        check(
            record,
            record["exit"] == 0 and record["versions"].get("choice") == "2.0",
            "extra index priority / union selection",
        )
    pip_record = run(
        "pip-index-union",
        lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
        setup=indexes,
        pip=True,
    )
    check(
        pip_record,
        pip_record["exit"] == 0 and pip_record["versions"].get("choice") == "2.0",
        "public pip all-wheel index union",
    )

    def reverse_indexes(f):
        f.add("choice", "2.0", index="one")
        f.add("choice", "1.0", index="two")

    for strategy in (None, "unsafe-best-match"):
        record = run(
            "index-reverse-" + (strategy or "default"),
            lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
            setup=reverse_indexes,
            extra=() if strategy is None else ("--index-strategy", strategy),
        )
        expected = "1.0" if strategy is None else "2.0"
        check(
            record,
            record["exit"] == 0 and record["versions"].get("choice") == expected,
            "measured default priority vs pip union",
        )
    pip_record = run(
        "pip-index-reverse",
        lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
        setup=reverse_indexes,
        pip=True,
    )
    check(
        pip_record,
        pip_record["exit"] == 0 and pip_record["versions"].get("choice") == "2.0",
        "pip highest across indexes",
    )

    for strategy in (None, "allow", "disallow"):
        record = run(
            "prerelease-" + (strategy or "default"),
            "preview>1.0",
            setup=lambda f: f.add("preview", "2.0a1"),
            extra=() if strategy is None else ("--prerelease", strategy),
        )
        check(
            record,
            (record["exit"] == 0) == (strategy != "disallow"),
            "uv prerelease default/explicit policy",
        )
    pip_record = run(
        "pip-prerelease", "preview>1.0", setup=lambda f: f.add("preview", "2.0a1"), pip=True
    )
    check(
        pip_record,
        pip_record["exit"] == 0 and pip_record["versions"] == {"preview": "2.0a1"},
        "current pip26.2.1 agrees on prerelease-only fixture",
    )

    def tied(f):
        f.add("choice", index="one", requires=["one-leaf==1.0"])
        f.add("choice", index="two", requires=["two-leaf==1.0"])
        f.add("one-leaf", index="one")
        f.add("two-leaf", index="two")

    record = run(
        "equal-version-index-identity-uv",
        lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
        setup=tied,
        extra=("--index-strategy", "unsafe-best-match"),
    )
    check(record, record["exit"] == 0, "equal-version artifact identity explicitly measured")
    record = run(
        "equal-version-index-identity-pip",
        lambda f: "--extra-index-url " + f.base + "/two/simple/\nchoice",
        setup=tied,
        pip=True,
    )
    check(record, record["exit"] == 0, "public pip equal-version artifact identity measured")

    def candidates(f):
        f.add("choice", "1.0")
        f.add("choice", "2.0")

    first = run("deterministic-first", "choice", setup=candidates, format="requirements.txt")
    second = run("deterministic-second", "choice", setup=candidates, format="requirements.txt")
    check(
        first,
        first["exit"] == second["exit"] == 0 and first["output"] == second["output"],
        "identical named lock text across fresh runs",
    )
    seed = "choice==1.0\n"
    pinned = run(
        "existing-output-default", "choice", setup=candidates, format="requirements.txt", seed=seed
    )
    upgraded = run(
        "existing-output-upgrade",
        "choice",
        setup=candidates,
        format="requirements.txt",
        seed=seed,
        extra=("--upgrade",),
    )
    check(
        pinned, "choice==1.0" in pinned["output"], "existing output implicitly constrains compile"
    )
    check(
        upgraded,
        "choice==2.0" in upgraded["output"],
        "explicit upgrade removes old output preference",
    )

    def missing(f):
        f.add("root", "1.0", requires=["absent==1.0"])
        f.add("root", "0.9")

    record = run("missing-closure-no-version-fallback", "root==1.0", setup=missing)
    check(
        record,
        record["exit"] != 0
        and record["output"] is None
        and not any("root-0.9-" in r["path"] for r in record["requests"]),
        "fixed root never substituted",
    )

    def unavailable(f):
        f.responses["/one/simple/root/"] = (503, "text/plain", b"")
        f.add("root", "0.9", index="two")

    record = run("unavailable-index-no-fallback", "root==1.0", setup=unavailable)
    check(
        record,
        record["exit"] != 0
        and record["output"] is None
        and not any(r["path"].startswith("/two/") for r in record["requests"]),
        "operational failure never invokes alternate index",
    )

    def priority_unavailable(f):
        f.add("root")
        f.responses["/two/simple/root/"] = (503, "text/plain", b"")

    for strategy in ("first-index", "unsafe-best-match"):
        record = run(
            "priority-index-503-" + strategy,
            lambda f: "--extra-index-url " + f.base + "/two/simple/\nroot==1.0",
            setup=priority_unavailable,
            extra=("--index-strategy", strategy),
        )
        check(
            record,
            record["exit"] != 0 and record["output"] is None,
            "index operational failure does not silently yield a lower-priority closure",
        )

    (args.output / "summary.json").write_text(
        json.dumps(
            {
                "cases": len(evaluation.records),
                "failed_assertions": evaluation.failures,
                "versions": {r["case"]: r["versions"] for r in evaluation.records},
                "exits": {r["case"]: r["exit"] for r in evaluation.records},
            },
            indent=2,
        )
        + "\n"
    )
    print(
        json.dumps({"cases": len(evaluation.records), "failed_assertions": evaluation.failures}),
        flush=True,
    )
    raise SystemExit(bool(evaluation.failures))


if __name__ == "__main__":
    main()
