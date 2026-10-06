"""Evidence-only adapter: shared acquisition first, public offline uv second.

No pip report conversion, source execution, installation, private pip/uv API,
or remote metadata fetching. The finite universe is an explicit owner grant.
"""
import argparse
import base64
import csv
from email.parser import BytesParser
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import threading
import tomllib
from urllib.parse import unquote, urlparse
import zipfile

from packaging.markers import default_environment
from packaging.metadata import Metadata
from packaging.requirements import Requirement
from packaging.tags import compatible_tags, cpython_tags, parse_tag
from packaging.utils import canonicalize_name, parse_wheel_filename
from packaging.version import Version


class Refused(ValueError):
    pass


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def context(target):
    win = target == "windows"
    environment = default_environment()
    environment.update(python_version="3.12", python_full_version="3.12.0",
                       implementation_version="3.12.0", implementation_name="cpython",
                       platform_python_implementation="CPython", platform_machine="AMD64" if win else "x86_64",
                       sys_platform="win32" if win else "linux", os_name="nt" if win else "posix",
                       platform_system="Windows" if win else "Linux")
    platforms = ["win_amd64"] if win else ["manylinux_2_17_x86_64", "linux_x86_64"]
    tags = set(cpython_tags((3, 12), abis=["cp312"], platforms=platforms))
    tags.update(compatible_tags((3, 12), interpreter="cp312", platforms=platforms))
    return environment, tags


def requirements(metadata):
    result = [raw if isinstance(raw, Requirement) else Requirement(raw) for raw in metadata.requires_dist or []]
    require(all(r.url is None for r in result), "dependency URL/VCS refused before uv, including inactive markers")
    return result


def inspect_wheel(item, directory, tags):
    name, version, _build, filename_tags = parse_wheel_filename(item["filename"])
    path = directory / item["filename"]
    require(path.is_file() and not path.is_symlink(), "catalog member must be regular")
    with path.open("rb") as source:
        data = source.read(1024 * 1024 + 1)
    require(len(data) <= 1024 * 1024, "fixture wheel exceeds bounded inspection size")
    require(len(data) == item["bytes"] and digest(data) == item["sha256"], "actual wheel size/hash mismatch")
    require(name == canonicalize_name(item["name"]) and version == Version(item["version"]), "owner filename identity mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        members = archive.infolist()
        require(len(members) <= 128 and len({m.filename for m in members}) == len(members), "duplicate/excess wheel entries")
        for member in members:
            require(not member.filename.startswith("/") and ".." not in Path(member.filename).parts,
                    "unsafe wheel member")
            require(not stat.S_ISLNK(member.external_attr >> 16), "wheel symlink refused")
            require(member.file_size <= 1024 * 1024, "excess expanded fixture member")
        metadata_members = [m.filename for m in members if m.filename.endswith(".dist-info/METADATA")]
        wheel_members = [m.filename for m in members if m.filename.endswith(".dist-info/WHEEL")]
        require(len(metadata_members) == len(wheel_members) == 1, "unique METADATA/WHEEL required")
        prefix = metadata_members[0].rsplit("/", 1)[0]
        require(wheel_members[0] == prefix + "/WHEEL", "METADATA/WHEEL namespace mismatch")
        distribution, info_version = prefix.removesuffix(".dist-info").rsplit("-", 1)
        require(canonicalize_name(distribution) == name and Version(info_version) == version,
                "actual dist-info namespace identity mismatch")
        metadata = Metadata.from_email(archive.read(metadata_members[0]), validate=True)
        require(canonicalize_name(metadata.name) == name and metadata.version == version,
                "actual METADATA identity mismatch")
        wheel = BytesParser().parsebytes(archive.read(wheel_members[0]))
        require(wheel["Wheel-Version"] == "1.0", "unsupported WHEEL version")
        declared_tags = set().union(*(parse_tag(t) for t in wheel.get_all("Tag", [])))
        require(declared_tags == filename_tags, "actual WHEEL tags disagree with filename")
        rows = list(csv.reader(io.StringIO(archive.read(prefix + "/RECORD").decode())))
        require(len(rows) == len(members) and {r[0] for r in rows} == {m.filename for m in members}, "fixture RECORD coverage")
        for member, hash_value, size in rows:
            if member.endswith("/RECORD"):
                require(not hash_value and not size, "RECORD self hash")
            else:
                body = archive.read(member)
                expected = base64.urlsafe_b64encode(hashlib.sha256(body).digest()).rstrip(b"=").decode()
                require(hash_value == "sha256=" + expected and size == str(len(body)), "actual fixture RECORD mismatch")
    dependencies = requirements(metadata)
    eligible = bool(tags & filename_tags) and (not metadata.requires_python or metadata.requires_python.contains("3.12.0"))
    return {**item, "local": str(path), "eligible": eligible, "metadata": metadata,
            "requirements": dependencies, "tags": sorted(map(str, filename_tags))}


def active(requirement, extras, environment):
    return requirement.marker is None or any(requirement.marker.evaluate({**environment, "extra": extra}) for extra in {"", *extras})


def bounded_coverage(roots, candidates, environment):
    """Conservative closure coverage, not a new dependency solver.

    Traverse every eligible version for reachable names and union requested
    extras. Require an eligible catalog candidate for each active requirement.
    This can refuse an otherwise satisfiable universe; it never widens one.
    """
    reached = {}
    queue = list(roots)
    while queue:
        requirement = queue.pop()
        if not active(requirement, (), environment):
            continue
        name = canonicalize_name(requirement.name)
        eligible = [c for c in candidates if c["eligible"] and canonicalize_name(c["name"]) == name
                    and (not requirement.specifier or requirement.specifier.contains(c["version"], prereleases=True))]
        require(eligible, f"incomplete bounded catalog: {requirement}")
        extras = reached.get(name)
        incoming = set(requirement.extras)
        if extras is not None and incoming <= extras:
            continue
        reached.setdefault(name, set()).update(incoming)
        # All versions for this name must be covered, including rejected versions.
        for candidate in candidates:
            if candidate["eligible"] and canonicalize_name(candidate["name"]) == name:
                for dependency in candidate["requirements"]:
                    if active(dependency, reached[name], environment):
                        queue.append(Requirement(str(dependency).split(";", 1)[0]))
    return {name: sorted(extras) for name, extras in sorted(reached.items())}


def isolated_environment(directory):
    env = {"PATH": str(directory / "sentinels") + ":/usr/bin:/bin", "LANG": "C.UTF-8",
           "HOME": str(directory / "home"), "XDG_CONFIG_HOME": str(directory / "config"),
           "XDG_DATA_HOME": str(directory / "data"), "XDG_CACHE_HOME": str(directory / "cache"),
           "UV_CACHE_DIR": str(directory / "cache"), "UV_CREDENTIALS_DIR": str(directory / "credentials"),
           "TMPDIR": str(directory / "temporary"),
           "UV_PYTHON_DOWNLOADS": "never", "NETRC": os.devnull, "PIP_CONFIG_FILE": os.devnull,
           "UV_HTTP_RETRIES": "0"}
    for entry in ("home", "config", "data", "cache", "credentials", "sentinels", "temporary"):
        (directory / entry).mkdir(exist_ok=True)
    for name in ("git", "pip", "python", "python3"):
        # These are experiment-owned refusal sentinels, never package hooks.
        path = directory / "sentinels" / name
        path.write_text("#!/bin/sh\nprintf '%s\\n' 'refused tool " + name + "' >> " + str(directory / "tool-attempts.log") + "\nexit 97\n")
        path.chmod(0o700)
    return env


def invoke(command, directory, env):
    traced = ["/usr/bin/strace", "-f", "-e", "trace=network,execve", "-o", str(directory / "uv.strace"), *command]
    process = subprocess.Popen(traced, cwd=directory, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=30)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        raise Refused("bounded offline solver timed out; no fallback") from None
    (directory / "uv.stdout").write_bytes(stdout)
    (directory / "uv.stderr").write_bytes(stderr)
    trace = (directory / "uv.strace").read_text()
    network = [line for line in trace.splitlines() if re.search(r"(socket|connect|sendto|sendmsg|sendmmsg)\(.*AF_INET", line)]
    executions = [line for line in trace.splitlines() if "execve(" in line]
    allowed = {command[0], sys.executable}
    require(not network, "offline uv attempted IP network")
    require(all(any('execve("' + executable + '"' in line for executable in allowed) for line in executions),
            "unexpected tool/hook execution")
    require(not (directory / "tool-attempts.log").exists(), "unexpected resolver tool attempt")
    return process.returncode, {"ip_operations": network, "executions": executions}


def selected_packet(lock, candidates, directory, environment, roots):
    require(lock.get("lock-version") == "1.0" and lock.get("created-by") == "uv", "unsupported public lock schema")
    selected = []
    for package in lock.get("packages", []):
        require(not any(k in package for k in ("sdist", "vcs", "directory")), "non-wheel lock source")
        if package.get("marker"):
            from packaging.markers import Marker
            if not Marker(package["marker"]).evaluate(environment):
                continue
        entries = package.get("wheels") or [package.get("archive")]
        matches = []
        for entry in entries:
            require(isinstance(entry, dict), "missing wheel lock entry")
            require(entry.get("hashes", {}).get("sha256"), "missing selected hash")
            raw = entry.get("url") or entry.get("path")
            require(isinstance(raw, str), "missing selected local locator")
            parsed = urlparse(raw)
            require(parsed.scheme in ("", "file") and not parsed.netloc, "lock escaped local projection")
            local = Path(unquote(parsed.path))
            if not local.is_absolute():
                local = directory / local
            local = local.resolve()
            matching = [c for c in candidates if Path(c["local"]).resolve() == local
                        and c["sha256"] == entry["hashes"]["sha256"]
                        and canonicalize_name(c["name"]) == package["name"] and c["version"] == package["version"]]
            require(len(matching) == 1, "lock member has no exact original-source mapping")
            if matching[0]["eligible"]:
                matches.extend(matching)
        require(len(matches) == 1, "ambiguous/ineligible exact selected wheel")
        selected.append(matches[0])
    require(len({canonicalize_name(c["name"]) for c in selected}) == len(selected), "duplicate selected package")
    by_name = {canonicalize_name(c["name"]): c for c in selected}
    queue = list(roots)
    reached = {}
    while queue:
        requirement = queue.pop()
        if not active(requirement, (), environment):
            continue
        name = canonicalize_name(requirement.name)
        require(name in by_name and requirement.specifier.contains(by_name[name]["version"], prereleases=True),
                "selected closure does not satisfy actual requirements")
        incoming = set(requirement.extras)
        require(incoming <= set(by_name[name]["metadata"].provides_extra or []), "selected extras undeclared")
        extras = reached.get(name)
        if extras is not None and incoming <= extras:
            continue
        reached.setdefault(name, set()).update(incoming)
        for dependency in by_name[name]["requirements"]:
            if active(dependency, reached[name], environment):
                queue.append(Requirement(str(dependency).split(";", 1)[0]))
    require(set(reached) == set(by_name), "selected lock has unrelated packages")
    return [{k: c[k] for k in ("id", "name", "version", "filename", "url", "sha256", "bytes", "local", "tags")} for c in selected]


def inspect(spec_path, uv, wheels):
    directory = spec_path.parent
    spec = json.loads(spec_path.read_text())
    outcome = {"status": "preflight_refused", "uv_invoked": False}
    try:
        require(spec["catalog_digest"] == digest(canonical(spec["candidates"])), "owner catalog declaration changed")
        require(spec["complete_declaration"] is True, "owner has not declared finite catalog complete")
        expected = {c["filename"] for c in spec["candidates"]}
        require({p.name for p in wheels.iterdir()} == expected, "unexpected/incomplete local catalog namespace")
        environment, tags = context(spec["target"])
        candidates = [inspect_wheel(c, wheels, tags) for c in spec["candidates"]]
        roots = []
        local_roots = []
        original_roots = []
        for raw in spec["roots"]:
            require("\n" not in raw and "\r" not in raw, "requirement option injection")
            root = Requirement(raw)
            if root.url:
                matches = [c for c in candidates if root.url == c["url"] + "#sha256=" + c["sha256"]
                           and canonicalize_name(root.name) == canonicalize_name(c["name"])]
                require(len(matches) == 1 and matches[0]["eligible"], "root source is not exact owner wheel")
                item = matches[0]
                extra = "[" + ",".join(sorted(root.extras)) + "]" if root.extras else ""
                marker = " ; " + str(root.marker) if root.marker else ""
                local_roots.append(root.name + extra + " @ " + Path(item["local"]).as_uri() + "#sha256=" + item["sha256"] + marker)
                roots.append(Requirement(root.name + extra + "==" + item["version"] + marker))
                original_roots.append({"original_requirement": raw, "original_url": item["url"], "sha256": item["sha256"], "candidate_id": item["id"]})
            else:
                roots.append(root)
                local_roots.append(str(root))
                original_roots.append({"original_requirement": raw})
        coverage = bounded_coverage(roots, candidates, environment)
        projection = {"schema": "pumas.experiment.offline-input.v1", "catalog_digest": spec["catalog_digest"],
                      "target_environment": environment, "original_roots": original_roots,
                      "local_requirements": local_roots, "conservative_coverage": coverage,
                      "candidates": [{k: c[k] for k in ("id", "url", "sha256", "bytes", "filename", "local", "tags", "eligible")} for c in candidates]}
        (directory / "projection.json").write_text(json.dumps(projection, indent=2) + "\n")
        requirements_path = directory / "requirements.in"
        requirements_path.write_text("\n".join(local_roots) + "\n")
        output = directory / "pylock.toml"
        require(not output.exists(), "solver output must be fresh")
        command = [str(uv), "--no-config", "--no-cache", "--offline", "--no-python-downloads", "--no-managed-python",
                   "--color", "never", "pip", "compile", str(requirements_path), "--no-index", "--find-links", str(wheels),
                   "--no-build", "--no-sources", "--keyring-provider", "disabled",
                   "--python", str(directory / "missing-owned-python") if spec.get("solver_fault") == "missing-python" else sys.executable,
                   "--python-version", "3.12", "--python-platform",
                   "x86_64-pc-windows-msvc" if spec["target"] == "windows" else "x86_64-manylinux_2_17",
                   "--format", "pylock.toml", "--generate-hashes", "--no-header", "--no-annotate", "-o", str(output)]
        env = isolated_environment(directory)
        (directory / "invocation.json").write_text(json.dumps({"argv": command, "environment": env}, indent=2) + "\n")
        outcome["uv_invoked"] = True
        exit_code, observation = invoke(command, directory, env)
        outcome.update(exit=exit_code, observation=observation)
        require(exit_code == 0, "resolver nonzero exit is fatal/inconclusive; no fallback")
        # Reinspect actual bytes after solver; owned namespace/provenance must stay exact.
        require({p.name for p in wheels.iterdir()} == expected, "solver changed catalog namespace")
        for item in spec["candidates"]:
            inspect_wheel(item, wheels, tags)
        lock = tomllib.loads(output.read_text())
        selected = selected_packet(lock, candidates, directory, environment, roots)
        outcome.update(status="selected", selected=selected, lock_sha256=digest(output.read_bytes()), projection_sha256=digest((directory / "projection.json").read_bytes()))
    except (ValueError, KeyError, zipfile.BadZipFile, ExceptionGroup) as error:
        outcome["reason"] = str(error)
        if outcome["uv_invoked"]:
            outcome["status"] = "solver_or_mapping_refused"
    print(json.dumps(outcome))


class Source:
    def __init__(self):
        self.routes = {}
        self.requests = []
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                owner.requests.append({"method": "GET", "path": self.path, "authorization": bool(self.headers.get("Authorization"))})
                status, headers, body = owner.routes.get(self.path, (451, {}, b"owner did not admit this route"))
                self.send_response(status)
                self.send_header("Content-Length", str(len(body)))
                for name, value in headers.items():
                    self.send_header(name, value)
                self.end_headers()
                self.wfile.write(body)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.base = f"http://127.0.0.1:{self.server.server_port}"
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.thread.start()

    def close(self):
        self.server.shutdown()
        self.thread.join()
        self.server.server_close()


def evaluate(harness, uv, output):
    source_module = Path(__file__).parent.parent / "torch-public-uv-evaluation-2026-10-06/evaluate_uv.py"
    module_spec = importlib.util.spec_from_file_location("inert_wheel_factory", source_module)
    factory = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(factory)
    root = Path(__file__).resolve().parents[5]
    packaging = root / "torch-server/tooling/packaging.zip"
    records = []

    def run(name, make, *, expected=None, refused=None, target="linux", roots=None, modify=None,
            reason=None, expected_exit=None):
        directory = output / name
        directory.mkdir(parents=True)
        source = Source()
        candidates = []
        allowed = []

        def add(project, version="1.0", *, redirect=None, mutate=None, **kwargs):
            filename, body = factory.wheel(project, version, **kwargs)
            if mutate:
                body = mutate(body)
            path = f"/owner/{filename}"
            url = source.base + path
            source.routes[path] = (200, {}, body)
            allowed.append(url)
            if redirect:
                destination = "/approved/" + filename if redirect == "approved" else "/trap/unapproved.whl"
                source.routes[path] = (302, {"Location": source.base + destination}, b"")
                source.routes[destination] = (200, {}, body)
                if redirect == "approved":
                    allowed.append(source.base + destination)
            item = {"id": f"candidate-{len(candidates)}", "name": project, "version": version,
                    "filename": filename, "url": url, "sha256": digest(body), "bytes": len(body)}
            candidates.append(item)
            return project + " @ " + url + "#sha256=" + item["sha256"]

        returned_roots = make(add, source)
        spec = {"case": name, "candidates": candidates, "approved_urls": allowed, "target": target,
                "complete_declaration": True, "roots": roots or returned_roots or ["root"],
                "catalog_digest": digest(canonical(candidates))}
        if modify:
            modify(spec, source, directory)
        declaration = (json.dumps(spec, indent=2) + "\n").encode()
        (directory / "owner-spec.json").write_bytes(declaration)
        env = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
        try:
            completed = subprocess.run([str(harness), str(directory / "owner-spec.json"), sys.executable,
                                        str(Path(__file__).resolve()), str(uv), str(packaging)],
                                       env=env, capture_output=True, timeout=90)
            (directory / "harness.stdout").write_bytes(completed.stdout)
            (directory / "harness.stderr").write_bytes(completed.stderr)
            outcome = json.loads((directory / "outcome.json").read_text())
            payload = outcome.get("payload", {})
            versions = {c["name"]: c["version"] for c in payload.get("selected", [])}
            checks = {"harness_success": completed.returncode == 0,
                      "only_admitted_requests": all(source.base + r["path"] in allowed for r in source.requests),
                      "no_credential_headers": all(not r["authorization"] for r in source.requests),
                      "immutable_owner_provenance": (directory / "owner-spec.json").read_bytes() == declaration}
            if (directory / "completion-receipt.json").exists():
                receipt = json.loads((directory / "completion-receipt.json").read_text())
                held = payload["held_use_record"]
                checks["exact_consumer_receipt"] = (receipt["manifest"] == held["manifest"]
                    and receipt["verified_files"] == held["files"] and receipt["acquisition_id"] == held["id"]
                    and receipt["use_lease"] == held["phase"]["lease"]
                    and receipt["demand"]["operation"] == name
                    and [f["sha256"] for f in receipt["verified_files"]] == [c["sha256"] for c in candidates])
            if expected is not None:
                checks.update(selected=payload.get("status") == "selected" and versions == expected,
                              held_using=payload.get("held_use_record", {}).get("phase", {}).get("state") == "using",
                              settled_adopted=all(r["phase"]["state"] == "adopted" for r in outcome["settled_records"]))
            if refused == "before_uv":
                checks["refused_before_uv"] = payload.get("status") == "preflight_refused" and not payload.get("uv_invoked")
            elif refused == "acquisition":
                checks["refused_before_inspection"] = outcome["status"] in ("owner_source_refused", "acquisition_or_inspection_refused") and not (directory / "invocation.json").exists()
            elif refused == "solver":
                checks["solver_no_fallback"] = payload.get("status") == "solver_or_mapping_refused" and payload.get("uv_invoked")
            if reason:
                checks["specific_refusal_reason"] = reason in payload.get("reason", outcome.get("error", ""))
            if expected_exit is not None:
                checks["specific_exit"] = payload.get("exit") == expected_exit
            if payload.get("uv_invoked"):
                checks["observed_no_uv_ip_io"] = payload.get("observation", {}).get("ip_operations") == []
                checks["one_uv_invocation"] = len(payload.get("observation", {}).get("executions", [])) in (1, 2)
            if (directory / "projection.json").exists():
                projection = json.loads((directory / "projection.json").read_text())
                checks["exact_projection_provenance"] = all(
                    original["url"] == projected["url"] and original["sha256"] == projected["sha256"]
                    for original, projected in zip(candidates, projection["candidates"], strict=True))
            if name.startswith("primed-cache"):
                prime = json.loads((directory / "priming/evidence.json").read_text())
                checks["actual_public_uv_cache_prime"] = prime["exit"] == 0 and prime["version"] == "9.0" and prime["cache_files"] > 0
            checks["no_install"] = not any(p.name in ("site-packages", "pyvenv.cfg") for p in directory.rglob("*"))
            record = {"case": name, "checks": checks, "outcome": outcome, "requests": source.requests,
                      "failed_assertions": [key for key, value in checks.items() if not value]}
            (directory / "requests.json").write_text(json.dumps(source.requests, indent=2) + "\n")
            records.append(record)
            print(json.dumps({"case": name, "failed": record["failed_assertions"], "status": payload.get("status", outcome["status"])}), flush=True)
        finally:
            source.close()

    def closure(add, _source):
        direct = add("root", requires=("leaf[tools]>=1,<3", 'winleaf; sys_platform == "win32"', 'linuxleaf; sys_platform == "linux"'))
        add("leaf", "1.0", requires=('helper==1.0; extra == "tools"',), extras=("tools",))
        add("leaf", "2.0", requires=('helper==1.0; extra == "tools"',), extras=("tools",))
        add("leaf", "3.0", requires=('helper==1.0; extra == "tools"',), extras=("tools",))
        add("helper")
        add("winleaf", tag="cp312-cp312-win_amd64")
        add("linuxleaf", tag="cp312-cp312-manylinux_2_17_x86_64")
        return [direct]

    run("multi-extras-linux", closure, expected={"root": "1.0", "leaf": "2.0", "helper": "1.0", "linuxleaf": "1.0"})
    run("multi-extras-windows", closure, target="windows", expected={"root": "1.0", "leaf": "2.0", "helper": "1.0", "winleaf": "1.0"})
    run("approved-redirect", lambda add, _: [add("root", redirect="approved")], expected={"root": "1.0"})
    run("denied-redirect", lambda add, _: [add("root", redirect="denied")], refused="acquisition")
    run("missing-candidate", lambda add, _: [add("root", requires=("missing>=1",))], refused="before_uv", reason="incomplete bounded catalog")
    run("undeclared-completeness", lambda add, _: [add("root")], refused="before_uv",
        modify=lambda spec, _source, _directory: spec.update(complete_declaration=False))
    for name, requirement in (("source-url", "leaf @ {base}/trap/source.tar.gz"),
                              ("vcs-url", "leaf @ git+{base}/trap/repo.git"),
                              ("inactive-url", 'leaf @ {base}/trap/source.tar.gz ; python_version < "2.0"')):
        run(name, lambda add, source, requirement=requirement: [add("root", requires=(requirement.format(base=source.base),))], refused="before_uv", reason="dependency URL/VCS refused")
    def unselected_url(add, source):
        direct = add("root", requires=("leaf==1.0",))
        add("leaf", "1.0")
        add("leaf", "9.0", requires=("trap @ " + source.base + "/trap/source.tar.gz",))
        return [direct]
    run("unselected-candidate-url", unselected_url, refused="before_uv", reason="dependency URL/VCS refused")
    run("root-vcs", lambda add, source: (add("root"), ["root @ git+" + source.base + "/trap/repo.git"])[1], refused="before_uv", reason="root source is not exact owner wheel")
    run("root-option", lambda add, _: [add("root")], roots=["root\n--index-url http://127.0.0.1:1/"], refused="before_uv")
    def substitution(spec, source, _directory):
        item = spec["candidates"][0]
        path = urlparse(item["url"]).path
        _filename, wrong = factory.wheel("root", "9.0")
        source.routes[path] = (200, {}, wrong)
    run("source-substitution", lambda add, _: [add("root")], modify=substitution, refused="acquisition")
    def corrupt_metadata(data):
        archive_in = zipfile.ZipFile(io.BytesIO(data))
        archive_out = io.BytesIO()
        with zipfile.ZipFile(archive_out, "w") as archive:
            for member in archive_in.infolist():
                body = archive_in.read(member.filename)
                if member.filename.endswith("/METADATA"):
                    body = body.replace(b"Name: root", b"Name: wrong")
                archive.writestr(member, body)
        return archive_out.getvalue()
    run("actual-metadata-mismatch", lambda add, _: [add("root", mutate=corrupt_metadata)], refused="before_uv", reason="actual METADATA identity mismatch")
    def corrupt_tags(data):
        archive_in = zipfile.ZipFile(io.BytesIO(data))
        archive_out = io.BytesIO()
        with zipfile.ZipFile(archive_out, "w") as archive:
            for member in archive_in.infolist():
                body = archive_in.read(member.filename)
                if member.filename.endswith("/WHEEL"):
                    body = body.replace(b"py3-none-any", b"cp312-cp312-win_amd64")
                archive.writestr(member, body)
        return archive_out.getvalue()
    run("actual-wheel-tags-mismatch", lambda add, _: [add("root", mutate=corrupt_tags)], refused="before_uv", reason="actual WHEEL tags disagree")
    def python_tags(add, _):
        direct = add("root", requires=("choice",))
        add("choice", "1.0", tag="cp312-cp312-manylinux_2_17_x86_64", python=">=3.12")
        add("choice", "2.0", tag="cp311-cp311-manylinux_2_17_x86_64", python=">=3.11")
        add("choice", "3.0", python=">=3.13")
        return [direct]
    run("python-and-tags", python_tags, expected={"root": "1.0", "choice": "1.0"})
    def conflict(add, _):
        direct = add("root", requires=("left", "right"))
        add("left", requires=("leaf==1.0",))
        add("right", requires=("leaf==2.0",))
        add("leaf", "1.0")
        add("leaf", "2.0")
        return [direct]
    run("bounded-unsatisfiable", conflict, refused="solver", expected_exit=1)
    run("unclassified-operational-error", lambda add, _: [add("root")], refused="solver", expected_exit=2,
        modify=lambda spec, _source, _directory: spec.update(solver_fault="missing-python"))
    def compatible_inventory(add, _):
        direct = add("root", requires=("leaf",))
        add("leaf", tag="py3-none-any")
        add("leaf", tag="cp312-cp312-manylinux_2_17_x86_64")
        return [direct]
    run("ambiguous-compatible-files", compatible_inventory, refused="solver", expected_exit=0,
        reason="ambiguous/ineligible exact selected wheel")
    def unapproved_source(spec, source, _directory):
        spec["candidates"][0]["url"] = source.base + "/trap/unapproved.whl"
        spec["catalog_digest"] = digest(canonical(spec["candidates"]))
    run("unapproved-initial-source", lambda add, _: [add("root")], modify=unapproved_source, refused="acquisition")
    def unexpected_member(_spec, _source, directory):
        catalog = directory / "acquisition/catalog"
        catalog.mkdir(parents=True)
        filename, data = factory.wheel("unadmitted")
        (catalog / filename).write_bytes(data)
    run("unexpected-local-member", lambda add, _: [add("root")], modify=unexpected_member,
        refused="before_uv", reason="unexpected/incomplete local catalog namespace")
    def ambient(spec, source, directory):
        trap = source.base + "/trap/simple/"
        (directory / "uv.toml").write_text('index-url = "' + trap + '"\nfind-links = ["' + source.base + '/trap/wheels"]\n')
        (directory / ".python-version").write_text("9.99\n")
        (directory / "pyproject.toml").write_text('[project]\nname="ambient"\nversion="1"\ndependencies=["trap"]\n')
        # Parent poison is never inherited by the explicitly cleared acquisition/child environments.
        os.environ.update(UV_INDEX_URL=trap, UV_FIND_LINKS=source.base + "/trap/wheels",
                          UV_CACHE_DIR=str(directory / "ambient-cache"), HTTP_PROXY=source.base,
                          HTTPS_PROXY=source.base, PIP_INDEX_URL=trap, PYTHONPATH="/trap")
    run("ambient-config-env-python", closure, expected={"root": "1.0", "leaf": "2.0", "helper": "1.0", "linuxleaf": "1.0"}, modify=ambient)
    def prime_cache(spec, _source, directory):
        prime = directory / "priming"
        prime.mkdir()
        project = "leaf" if "missing" in spec["case"] else "root"
        primed_wheels = prime / "wheels"
        primed_wheels.mkdir()
        filename, data = factory.wheel(project, "9.0")
        (primed_wheels / filename).write_bytes(data)
        requirement = prime / "requirements.in"
        requirement.write_text(project + "\n")
        env = isolated_environment(prime)
        env["UV_CACHE_DIR"] = str(directory / "cache")
        command = [str(uv), "--no-config", "--offline", "--no-python-downloads", "--no-managed-python",
                   "pip", "compile", str(requirement), "--no-index", "--find-links", str(primed_wheels),
                   "--no-build", "--python", sys.executable, "--format", "pylock.toml", "--generate-hashes",
                   "-o", str(prime / "pylock.toml")]
        (prime / "invocation.json").write_text(json.dumps({"argv": command, "environment": env}, indent=2) + "\n")
        code, observation = invoke(command, prime, env)
        document = tomllib.loads((prime / "pylock.toml").read_text())
        (prime / "evidence.json").write_text(json.dumps({"exit": code, "observation": observation,
            "version": document["packages"][0]["version"],
            "cache_files": sum(p.is_file() for p in (directory / "cache").rglob("*"))}, indent=2) + "\n")
    run("primed-cache-source-substitution", lambda add, _: (add("root"), ["root"])[1],
        modify=prime_cache, expected={"root": "1.0"})
    run("primed-cache-missing-candidate", lambda add, _: [add("root", requires=("leaf",))],
        modify=prime_cache, refused="before_uv", reason="incomplete bounded catalog")
    (output / "evidence.json").write_text(json.dumps({"cases": records, "failed_assertions": sum(len(r["failed_assertions"]) for r in records)}, indent=2) + "\n")
    require(all(not record["failed_assertions"] for record in records), "one or more measured assertions failed")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "inspect":
        inspect(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]))
        return
    parser = argparse.ArgumentParser()
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--uv", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    evaluate(args.harness.resolve(), args.uv.resolve(), args.output.resolve())


if __name__ == "__main__":
    main()
