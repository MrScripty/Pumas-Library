"""Bounded catalog inspection, without network, resolver, builds or installation.

The Rust owner supplies complete finite project observations and held acquired
files. Output is evidence: JSON alone never grants acquisition or completeness.
All packaging operations use the repository-owned public standalone package.
"""
import argparse
from email.parser import Parser
import hashlib
import importlib
import importlib.util
import json
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import time
from urllib.parse import unquote, urlparse
import zipfile

spec = importlib.util.spec_from_file_location("catalog_target", Path(__file__).with_name("wheel_target.py"))
target_owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(target_owner)
packaging = importlib.import_module("packaging")
for module in ("metadata", "requirements", "tags", "utils", "version"):
    importlib.import_module("packaging." + module)

MAX_PROJECTS = 32
MAX_OBSERVATIONS = 64
MAX_PAGE = 1024 * 1024
MAX_METADATA = 8 * MAX_PAGE
MAX_ROWS = 4096
MAX_CANDIDATES = 128
MAX_EDGES = 4096
MAX_MEMBERS = 128
MAX_EXPANDED = 8 * MAX_PAGE
MAX_CATALOG_EXPANDED = 64 * MAX_PAGE
MAX_PAYLOAD = 64 * MAX_PAGE
MAX_SECONDS = 120
MAX_OUTPUT = 2 * MAX_PAGE
REQUEST_FIELDS = {"schema", "selection", "release", "build", "roots", "constraints",
                  "direct_roots", "target_observation_sha256", "observations"}


class Incomplete(ValueError):
    """No solver, incompatibility or fallback conclusion follows this result."""


class Refused(ValueError):
    pass


def check(value, reason):
    if not value:
        raise Refused(reason)


def bounded(value, maximum, reason):
    if value > maximum:
        raise Incomplete(reason)


def digest(value):
    return target_owner.observation_digest(value)


def decode(raw):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            check(key not in result, "Duplicate JSON field")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(Refused("Invalid JSON number")))


def safe_url(raw):
    check(isinstance(raw, str) and 0 < len(raw) <= 4096
          and not any(c.isspace() or ord(c) < 32 or ord(c) == 127 for c in raw), "Invalid source URL")
    url = urlparse(raw)
    check(url.scheme == "https" and url.hostname and url.port in (None, 443)
          and not url.username and not url.password and not url.query and not url.fragment,
          "Source URL is outside HTTPS authority")
    path = unquote(url.path)
    check("\\" not in path and not any(ord(c) < 32 for c in path)
          and not any(p in (".", "..") for p in path.split("/")), "Unsafe source path")
    return url, path


def payload_allowed(raw, name, build, target):
    url, path = safe_url(raw)
    pytorch = url.hostname in {"download.pytorch.org", "download-r2.pytorch.org"} and path.startswith("/whl/")
    pypi = url.hostname == "files.pythonhosted.org" and path.startswith("/packages/")
    if name in {"torch", "torchvision"}:
        check((pytorch and path.startswith(f"/whl/{build}/"))
              or (pypi and target.system == "macos" and build == "cpu"), "Torch wheel differs from selected source build")
    else:
        check(pytorch or pypi, "Wheel source is outside caller allowlist")
    return path.rsplit("/", 1)[-1]


def requirement(raw):
    check(isinstance(raw, str) and 0 < len(raw) <= 4096, "Invalid requirement")
    try:
        return packaging.requirements.Requirement(raw)
    except ValueError:
        raise Refused("Malformed requirement") from None


def active(req, target, extras=("",)):
    return not req.marker or any(req.marker.evaluate(target.marker_environment(extra)) for extra in extras)


def checked_target(request, observation):
    target = target_owner.checked_observation(observation)
    check(request.get("target_observation_sha256") == digest(observation), "Request target binding differs")
    check(observation["interpreter"] == sys.executable
          and observation["interpreter_sha256"] == target_owner.interpreter_digest(sys.executable),
          "Selected interpreter evidence changed")
    target.require_native_consumer()
    return target


def prepare(request, observation):
    """Admit a finite immutable provider catalog; never infer missing coverage."""
    check(type(request) is dict and request.keys() == REQUEST_FIELDS
          and request["schema"] == "pumas.wheel-catalog-request.v1", "Unknown catalog request")
    check(isinstance(request["selection"], str) and 0 < len(request["selection"]) <= 256, "Missing selection identity")
    release, build = request["release"], request["build"]
    check(isinstance(release, str) and re.fullmatch(r"\d+\.\d+\.\d+", release)
          and isinstance(build, str) and re.fullmatch(r"cpu|cu\d+|rocm\d+(?:\.\d+)+", build), "Unsupported release/build")
    target = checked_target(request, observation)
    check(type(request["roots"]) is list and 0 < len(request["roots"]) <= MAX_PROJECTS
          and type(request["constraints"]) is list and len(request["constraints"]) <= MAX_ROWS
          and type(request["direct_roots"]) is list and len(request["direct_roots"]) <= MAX_PROJECTS,
          "Invalid root/constraint collection")
    roots = [requirement(raw) for raw in request["roots"]]
    constraints = [requirement(raw) for raw in request["constraints"]]
    check(all(not r.url and not r.extras for r in constraints), "Unsupported constraint reference/extras")
    selected = release if target.system == "macos" and build == "cpu" else release + "+" + build
    torch_roots = [r for r in roots if packaging.utils.canonicalize_name(r.name) == "torch"]
    check(len(torch_roots) == 1 and active(torch_roots[0], target)
          and (torch_roots[0].url or str(torch_roots[0].specifier) == "==" + selected), "Root does not bind selected Torch release/build")
    direct = {}
    for row in request["direct_roots"]:
        check(type(row) is dict and row.keys() == {"name", "url", "sha256", "size"}, "Invalid direct-root identity")
        name = packaging.utils.canonicalize_name(row["name"])
        check(name not in direct and any(packaging.utils.canonicalize_name(r.name) == name and r.url == row["url"] for r in roots),
              "Direct-root identity does not belong to original roots")
        direct[name] = row
    check(all(not r.url or packaging.utils.canonicalize_name(r.name) in direct for r in roots), "Direct root lacks exact identity")
    observations = request["observations"]
    check(type(observations) is list, "Missing finite project observations")
    bounded(len(observations), MAX_OBSERVATIONS, "Observation budget exceeded")
    projects, coverage, candidates, exclusions, provenance = set(), set(), {}, [], []
    rows_count = metadata_bytes = 0
    for item in observations:
        check(type(item) is dict and item.keys() == {"repository", "project", "url", "status", "body", "observed_at"}, "Invalid project observation")
        repo, name = item["repository"], item["project"]
        check(repo in {"pytorch", "pypi"} and isinstance(name, str)
              and re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name), "Unknown repository/project authority")
        base = f"https://download.pytorch.org/whl/{build}/" if repo == "pytorch" else "https://pypi.org/simple/"
        check(item["url"] == base + name + "/", "Project response/redirect differs from receiving grant")
        check((repo, name) not in coverage, "Duplicate project observation")
        check(isinstance(item["observed_at"], str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", item["observed_at"]), "Missing observation time")
        raw = item["body"]
        check(isinstance(raw, str), "Missing raw project response")
        size = len(raw.encode("utf-8"))
        metadata_bytes += size
        bounded(size, MAX_PAGE, "Project response byte budget exceeded")
        bounded(metadata_bytes, MAX_METADATA, "Aggregate metadata budget exceeded")
        if item["status"] == 404 and not raw:
            rows = []
        elif item["status"] == 200:
            doc = decode(raw)
            check(type(doc) is dict and {"meta", "name", "files"} <= doc.keys()
                  and doc.keys() <= {"meta", "name", "files", "versions"}
                  and type(doc["meta"]) is dict and set(doc["meta"]) <= {"api-version", "_last-serial"}
                  and doc["meta"].get("api-version") in {"1.0", "1.1", "1.2", "1.3"}
                  and packaging.utils.canonicalize_name(doc["name"]) == name and type(doc["files"]) is list,
                  "Unsupported/incomplete Simple JSON response")
            api_version = doc["meta"]["api-version"]
            check(api_version == "1.0" or "versions" in doc, "Simple JSON 1.1+ requires versions")
            versions = None
            if "versions" in doc:
                declared = doc["versions"]
                check(type(declared) is list and all(isinstance(v, str) for v in declared),
                      "Invalid Simple versions collection")
                bounded(len(declared), MAX_ROWS, "Version list budget exceeded")
                check(len(declared) == len(set(declared)), "Duplicate Simple version string")
                versions = set()
                for version in declared:
                    try:
                        versions.add(packaging.version.Version(version))
                    except packaging.version.InvalidVersion:
                        pass  # Legacy strings/no-file versions are valid protocol data.
            rows = doc["files"]
        else:
            raise Incomplete("Project response does not prove complete enumeration or absence")
        projects.add(name)
        bounded(len(projects), MAX_PROJECTS, "Project budget exceeded")
        coverage.add((repo, name))
        provenance.append({"repository": repo, "project": name, "url": item["url"], "status": item["status"],
                           "observed_at": item["observed_at"], "body_sha256": hashlib.sha256(raw.encode()).hexdigest()})
        rows_count += len(rows)
        bounded(rows_count, MAX_ROWS, "Index row budget exceeded")
        for row in rows:
            check(type(row) is dict and {"filename", "url", "hashes"} <= row.keys()
                  and set(row) <= {"filename", "url", "hashes", "size", "requires-python", "yanked", "upload-time", "core-metadata", "dist-info-metadata", "gpg-sig", "provenance"},
                  "Unsupported candidate/index tracking fields")
            filename = row["filename"]
            check(isinstance(filename, str) and 0 < len(filename) <= 256, "Invalid candidate filename")
            if item["status"] == 200 and api_version != "1.0" and (
                    type(row.get("size")) is not int or row["size"] < 0):
                raise Incomplete("Simple JSON 1.1+ file lacks size evidence")
            if not filename.endswith(".whl"):
                exclusions.append({"repository": repo, "project": name, "filename": filename, "reason": "wheel-only"})
                continue
            parsed_name, version, _, tags = packaging.utils.parse_wheel_filename(filename)
            check(parsed_name == name, "Candidate project/filename mismatch")
            check(versions is None or version in versions, "Simple versions does not list wheel version")
            check(payload_allowed(row["url"], name, build, target) == filename, "Candidate URL/filename mismatch")
            restrictions = [r for r in roots + constraints if packaging.utils.canonicalize_name(r.name) == name and not r.url and active(r, target)]
            if not target.supports(tags) or any(not r.specifier.contains(version, prereleases=True) for r in restrictions):
                exclusions.append({"repository": repo, "project": name, "filename": filename, "reason": "target-or-explicit-constraint"})
                continue
            check(type(row["hashes"]) is dict and isinstance(row["hashes"].get("sha256"), str)
                  and re.fullmatch(r"[0-9a-f]{64}", row["hashes"]["sha256"]), "Candidate lacks original SHA256")
            if type(row.get("size")) is not int or row["size"] <= 0:
                raise Incomplete("Candidate lacks bounded size evidence")
            yanked = row.get("yanked", False)
            check(type(yanked) in {bool, str}, "Invalid yanked policy data")
            candidate = {"repository": repo, "name": name, "version": str(version), "filename": filename,
                         "url": row["url"], "sha256": row["hashes"]["sha256"], "size": row["size"], "yanked": yanked,
                         "requires_python_hint": row.get("requires-python")}
            identity = digest(candidate)
            check(identity not in candidates, "Duplicate candidate identity")
            candidates[identity] = {"id": identity, **candidate}
            bounded(len(candidates), MAX_CANDIDATES, "Candidate budget exceeded")
    for name, row in direct.items():
        matched = [c for c in candidates.values() if c["name"] == name and c["url"] == row["url"]
                   and c["sha256"] == row["sha256"] and c["size"] == row["size"]]
        check(len(matched) == 1, "Exact direct root is missing or ambiguous")
        if name == "torch":
            check(matched[0]["version"] == selected, "Direct root differs from selected Torch release/build")
    bounded(sum(c["size"] for c in candidates.values()), MAX_PAYLOAD, "Aggregate payload budget exceeded")
    result = {"schema": "pumas.wheel-catalog-plan.v1", "request_sha256": digest(request),
            "target_observation_sha256": digest(observation), "candidates": list(candidates.values()),
            "observations": provenance, "exclusions": exclusions}
    bounded(len(json.dumps(result, sort_keys=True, separators=(",", ":")).encode()), MAX_OUTPUT, "Catalog plan byte budget exceeded")
    return result


def inspect_candidate(candidate, path, target):
    check(not path.is_symlink() and path.is_file() and path.stat().st_size == candidate["size"], "Acquired candidate namespace/size differs")
    hashed = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(MAX_PAGE), b""):
            hashed.update(chunk)
    check(hashed.hexdigest() == candidate["sha256"], "Acquired candidate differs from original SHA256")
    _, _, _, filename_tags = packaging.utils.parse_wheel_filename(candidate["filename"])
    with zipfile.ZipFile(path) as archive:
        members = archive.infolist()
        bounded(len(members), MAX_MEMBERS, "ZIP member budget exceeded")
        bounded(sum(m.file_size for m in members), MAX_EXPANDED, "ZIP expanded byte budget exceeded")
        seen = set()
        for member in members:
            raw = member.filename
            parts = PurePosixPath(raw).parts
            check(raw and not raw.startswith("/") and "\\" not in raw and not any(ord(c) < 32 for c in raw)
                  and not any(p in {".", ".."} for p in raw.split("/")) and raw not in seen
                  and ":" not in raw and not member.flag_bits & 1
                  and stat.S_IFMT(member.external_attr >> 16) in {0, stat.S_IFREG, stat.S_IFDIR}, "Unsafe wheel ZIP namespace")
            check(parts and member.file_size >= 0, "Invalid wheel ZIP member")
            seen.add(raw)
        metadata = [m for m in members if m.filename.endswith(".dist-info/METADATA")]
        wheel = [m for m in members if m.filename.endswith(".dist-info/WHEEL")]
        check(len(metadata) == len(wheel) == 1 and metadata[0].filename.rsplit("/", 1)[0] == wheel[0].filename.rsplit("/", 1)[0], "Ambiguous wheel metadata identity")
        directory = metadata[0].filename.rsplit("/", 1)[0]
        identity = directory.removesuffix(".dist-info").split("-")
        check("/" not in directory and len(identity) == 2, "Wheel dist-info identity must be at archive root")
        info_name, info_version = identity
        normalized_version = packaging.version.Version(info_version)
        check(packaging.utils.canonicalize_name(info_name, validate=True) == candidate["name"]
              and str(normalized_version) == info_version
              and normalized_version == packaging.version.Version(candidate["version"]),
              "Wheel dist-info directory differs from distribution identity")
        info_roots = {m.filename.split("/", 1)[0] for m in members
                      if m.filename.split("/", 1)[0].endswith(".dist-info")}
        check(info_roots == {directory}, "Wheel contains additional dist-info identity")
        bounded(metadata[0].file_size, MAX_PAGE, "METADATA byte budget exceeded")
        bounded(wheel[0].file_size, 64 * 1024, "WHEEL byte budget exceeded")
        body = archive.read(metadata[0])
        try:
            packaging.metadata.Metadata.from_email(body, validate=True)
        except ExceptionGroup:
            raise Refused("Invalid wheel METADATA") from None
        document = Parser().parsestr(body.decode("utf-8"))
        wheel_document = Parser().parsestr(archive.read(wheel[0]).decode("utf-8"))
    check(len(document.get_all("Name", [])) == len(document.get_all("Version", [])) == 1
          and packaging.utils.canonicalize_name(document["Name"]) == candidate["name"]
          and str(packaging.version.Version(document["Version"])) == candidate["version"], "Actual metadata identity differs")
    check(len(wheel_document.get_all("Wheel-Version", [])) == 1
          and re.fullmatch(r"1\.\d+", wheel_document["Wheel-Version"]), "Unsupported WHEEL version")
    declared = set()
    for raw in wheel_document.get_all("Tag", []):
        declared.update(packaging.tags.parse_tag(raw))
    check(declared == filename_tags, "Actual WHEEL tags differ from filename")
    python = document.get_all("Requires-Python", [])
    check(len(python) <= 1, "Ambiguous Requires-Python")
    hint = candidate["requires_python_hint"]
    check(hint is None or (isinstance(hint, str) and python and
          str(target_owner.SpecifierSet(hint)) == str(target_owner.SpecifierSet(python[0]))),
          "Index Requires-Python contradicts actual wheel metadata")
    dependencies = document.get_all("Requires-Dist", [])
    bounded(len(dependencies), MAX_EDGES, "Candidate dependency budget exceeded")
    for raw in dependencies:
        check(not requirement(raw).url, "Dependency URL is unsupported, including inactive markers")
    return {"id": candidate["id"], "metadata_sha256": hashlib.sha256(body).hexdigest(),
            "metadata_bytes": len(body), "expanded_bytes": sum(m.file_size for m in members),
            "eligible": not python or target.allows_python(python[0]),
            "requires_python": python[0] if python else None, "dependencies": dependencies,
            "extras": sorted({packaging.utils.canonicalize_name(e) for e in document.get_all("Provides-Extra", [])})}


def inspect(request, observation, plan, wheels):
    start = time.monotonic()
    check(plan == prepare(request, observation), "Acquisition plan differs from original request")
    target = checked_target(request, observation)
    check(not wheels.is_symlink() and wheels.is_dir(), "Missing held catalog namespace")
    check({p.name for p in wheels.iterdir()} == {c["id"] for c in plan["candidates"]}, "Catalog namespace differs from admitted set")
    facts = {}
    for candidate in plan["candidates"]:
        directory = wheels / candidate["id"]
        check(not directory.is_symlink() and directory.is_dir()
              and {p.name for p in directory.iterdir()} == {candidate["filename"]}, "Candidate namespace differs")
        facts[candidate["id"]] = inspect_candidate(candidate, directory / candidate["filename"], target)
        bounded(sum(len(f["dependencies"]) for f in facts.values()), MAX_EDGES, "Aggregate dependency field budget exceeded")
        bounded(sum(f["metadata_bytes"] for f in facts.values()), MAX_METADATA, "Aggregate actual METADATA byte budget exceeded")
        bounded(sum(f["expanded_bytes"] for f in facts.values()), MAX_CATALOG_EXPANDED, "Aggregate expanded ZIP byte budget exceeded")
        bounded(time.monotonic() - start, MAX_SECONDS, "Inspection deadline exceeded")
    coverage = {(o["repository"], o["project"]) for o in plan["observations"]}
    reachable, extras, edges, absent = {}, {}, set(), set()
    def add(req):
        name = packaging.utils.canonicalize_name(req.name)
        values = reachable.setdefault(name, set())
        values.add(str(req))  # Union alternatives; never intersect candidate branches.
        extras.setdefault(name, {""}).update(packaging.utils.canonicalize_name(e) for e in req.extras)
        bounded(len(reachable), MAX_PROJECTS, "Reachable project budget exceeded")
        bounded(sum(len(v) for v in reachable.values()), MAX_EDGES, "Requirement budget exceeded")
    for raw in request["roots"]:
        req = requirement(raw)
        if active(req, target):
            add(req)
    changed = True
    while changed:
        before = digest({"reachable": {n: sorted(v) for n, v in reachable.items()}, "extras": {n: sorted(v) for n, v in extras.items()}})
        for name in list(reachable):
            if not {("pytorch", name), ("pypi", name)} <= coverage:
                raise Incomplete("Reachable project lacks a complete response from every approved repository")
            requirements = [requirement(raw) for raw in reachable[name]]
            eligible = [c for c in plan["candidates"] if c["name"] == name and facts[c["id"]]["eligible"]
                        and any((r.url == c["url"] if r.url else r.specifier.contains(c["version"], prereleases=True)) for r in requirements)]
            if not eligible:
                absent.add(name)  # Evidence only; this owner does not signal solver fallback.
            else:
                absent.discard(name)
            for candidate in eligible:
                fact = facts[candidate["id"]]
                # An alternative without a requested extra is not selected here;
                # enumerate its possible base/available-extra branches conservatively.
                enabled = extras[name] & ({""} | set(fact["extras"]))
                for raw in fact["dependencies"]:
                    req = requirement(raw)
                    if active(req, target, enabled):
                        edge = (candidate["id"], raw)
                        edges.add(edge)
                        bounded(len(edges), MAX_EDGES, "Dependency edge budget exceeded")
                        add(req)
        after = digest({"reachable": {n: sorted(v) for n, v in reachable.items()}, "extras": {n: sorted(v) for n, v in extras.items()}})
        changed = before != after
        bounded(time.monotonic() - start, MAX_SECONDS, "Enumeration deadline exceeded")
    checked_target(request, observation)
    result = {"schema": "pumas.wheel-catalog-evidence.v1", "request_sha256": plan["request_sha256"],
            "target_observation_sha256": plan["target_observation_sha256"], "candidates": plan["candidates"],
            "observations": plan["observations"], "exclusions": plan["exclusions"], "facts": list(facts.values()),
            "reachable": {n: sorted(v) for n, v in reachable.items()}, "extras": {n: sorted(v) for n, v in extras.items()},
            "edges": [list(edge) for edge in sorted(edges)], "empty_domains": sorted(absent)}
    bounded(len(json.dumps(result, sort_keys=True, separators=(",", ":")).encode()), MAX_OUTPUT, "Catalog evidence byte budget exceeded")
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--observation", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--wheels", type=Path)
    args = parser.parse_args()
    try:
        def read(path, maximum):
            check(not path.is_symlink() and path.is_file(), "Missing/linked owner input")
            bounded(path.stat().st_size, maximum, "Owner input byte budget exceeded")
            with path.open("rb") as stream:
                body = stream.read(maximum + 1)
            bounded(len(body), maximum, "Owner input byte budget exceeded")
            return decode(body)
        request = read(args.request, MAX_METADATA + 2 * MAX_PAGE)
        observation = read(args.observation, 64 * 1024)
        check((args.plan is None) == (args.wheels is None), "Partial inspection invocation")
        result = prepare(request, observation) if args.plan is None else inspect(
            request, observation, read(args.plan, 2 * MAX_PAGE), args.wheels)
        with args.output.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
    except Incomplete:
        print("Catalog enumeration incomplete", file=sys.stderr)
        raise SystemExit(20) from None
    except (ValueError, TypeError, KeyError, OSError, zipfile.BadZipFile, ExceptionGroup):
        print("Catalog request or acquired evidence refused", file=sys.stderr)
        raise SystemExit(21) from None


if __name__ == "__main__":
    main()
