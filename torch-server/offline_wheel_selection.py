"""Public offline solver projection and independent selected-wheel checking.

The Rust owner supplies a live CompleteCatalog. JSON here is evidence, never a
catalog capability. No subprocess, network, source build or installation occurs
in this helper. All requirements/markers/tags use owned public packaging APIs.
"""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import sys
import tokenize
import tomllib
from urllib.parse import unquote, urlparse
import zipfile

spec = importlib.util.spec_from_file_location("selection_catalog", Path(__file__).with_name("wheel_catalog_owner.py"))
catalog = importlib.util.module_from_spec(spec)
spec.loader.exec_module(catalog)
check = catalog.check
digest = catalog.digest
requirement = catalog.requirement
canonicalize = catalog.packaging.utils.canonicalize_name
Version = catalog.packaging.version.Version
Marker = __import__("packaging.markers", fromlist=["Marker"]).Marker


def resolver_platform(target):
    if target.system == "windows":
        return "x86_64-pc-windows-msvc"
    wire = target.to_dict()
    check(target.system == "linux" and wire["libc"]["family"] == "glibc"
          and wire["native_linux_tag"], "Target lacks qualified public solver projection")
    major, minor = map(int, wire["libc"]["version"].split("."))
    check(major == 2 and minor in {17, 28, *range(31, 41)}, "Unsupported public solver libc floor")
    return f"x86_64-manylinux_2_{minor}"


def checked_marker(marker):
    if marker:
        variables = {t.string for t in tokenize.generate_tokens(io.StringIO(str(marker)).readline)
                     if t.type == tokenize.NAME}
        check(variables <= catalog.target_owner.MARKER_KEYS | {"extra", "and", "or", "not", "in"}
              and not variables & {"platform_release", "platform_version"},
              "Marker cannot be projected without inspection-host leakage")


def checked_input(raw):
    check(isinstance(raw, str) and not any(c in raw for c in "\r\n\x00"), "Requirement option injection")
    req = requirement(raw)
    checked_marker(req.marker)
    return req


def prepare(request, observation, evidence, wheels):
    """Reinspect the entire granted acquired set, not solver/cache declarations."""
    plan = catalog.prepare(request, observation)
    actual = catalog.inspect(request, observation, plan, wheels)
    check(actual == evidence, "Complete catalog evidence changed")
    target = catalog.checked_target(request, observation)
    platform = resolver_platform(target)
    roots = [checked_input(raw) for raw in request["roots"]]
    constraints = [checked_input(raw) for raw in request["constraints"]]
    facts = {f["id"]: f for f in evidence["facts"]}
    for fact in facts.values():
        for raw in fact["dependencies"]:
            req = checked_input(raw)
            check(not req.url, "Dependency source reference refused")
    candidates = [{**c, "local": str((wheels / c["id"] / c["filename"]).absolute()),
                   "eligible": facts[c["id"]]["eligible"]} for c in evidence["candidates"]]
    # Local find-links cannot express provider yanking; never silently erase it.
    check(all(c["yanked"] is False for c in candidates if c["eligible"]), "Yanked source policy has no qualified local projection")
    local_roots, direct = [], []
    for raw, root in zip(request["roots"], roots):
        if root.url:
            authority = [d for d in request["direct_roots"] if canonicalize(d["name"]) == canonicalize(root.name)
                         and d["url"] == root.url]
            matches = [c for c in candidates if c["name"] == canonicalize(root.name) and c["url"] == root.url
                       and authority and c["sha256"] == authority[0]["sha256"] and c["size"] == authority[0]["size"]]
            check(len(matches) == 1 and matches[0]["eligible"], "Direct root lacks exact eligible acquired identity")
            item = matches[0]
            direct.append({"requirement": raw, "candidate_id": item["id"]})
            extras = "[" + ",".join(sorted(root.extras)) + "]" if root.extras else ""
            marker = " ; " + str(root.marker) if root.marker else ""
            local_roots.append(root.name + extras + " @ " + Path(item["local"]).as_uri()
                               + "#sha256=" + item["sha256"] + marker)
        else:
            local_roots.append(str(root))
    return {"schema": "pumas.offline-wheel-projection.v1", "request_sha256": digest(request),
            "catalog_sha256": digest(evidence), "target_observation_sha256": digest(observation),
            "python": target.python, "platform": platform, "interpreter": observation["interpreter"],
            "roots": local_roots, "constraints": list(map(str, constraints)), "direct_roots": direct,
            "find_links": [str(Path(c["local"]).parent) for c in candidates if c["eligible"]],
            "candidates": candidates}


def selected_packet(request, observation, evidence, projection, lock, directory):
    """Treat every public lock field as untrusted until independently mapped."""
    target = catalog.checked_target(request, observation)
    check(type(lock) is dict and lock.get("lock-version") == "1.0" and lock.get("created-by") == "uv"
          and set(lock) <= {"lock-version", "created-by", "requires-python", "packages"}, "Unsupported public lock profile")
    check(isinstance(lock.get("requires-python"), str) and target.allows_python(lock["requires-python"]),
          "Lock Python target differs")
    packages = lock.get("packages")
    check(type(packages) is list and len(packages) <= catalog.MAX_CANDIDATES, "Missing/bounded selected packages")
    candidates = projection["candidates"]
    selected = {}
    for package in packages:
        check(type(package) is dict and {"name", "version"} <= package.keys()
              and set(package) <= {"name", "version", "marker", "wheels", "archive"}, "Non-wheel/unknown selected source")
        check(type(package["name"]) is str and canonicalize(package["name"], validate=True) == package["name"]
              and type(package["version"]) is str, "Malformed selected distribution identity")
        marker = Marker(package["marker"]) if "marker" in package else None
        checked_marker(marker)
        enabled = not marker or marker.evaluate(target.marker_environment())
        check(("wheels" in package) != ("archive" in package), "Ambiguous public lock source")
        entries = package["wheels"] if "wheels" in package else [package["archive"]]
        check(type(entries) is list and 0 < len(entries) <= catalog.MAX_CANDIDATES, "Missing/bounded wheel entries")
        matches = []
        for entry in entries:
            check(type(entry) is dict and set(entry) <= {"url", "path", "hashes", "size"}
                  and ("url" in entry) != ("path" in entry) and type(entry.get("hashes")) is dict
                  and set(entry["hashes"]) == {"sha256"}, "Invalid selected wheel identity")
            raw = entry.get("url", entry.get("path"))
            check(type(raw) is str, "Missing selected locator")
            parsed = urlparse(raw)
            check(parsed.scheme in {"", "file"} and not parsed.netloc and not parsed.query and not parsed.fragment,
                  "Lock escaped local projection")
            local = Path(unquote(parsed.path))
            if not local.is_absolute():
                local = directory / local
            matched = [c for c in candidates if Path(c["local"]) == local.resolve()
                       and c["sha256"] == entry["hashes"]["sha256"] and c["name"] == package["name"]
                       and Version(c["version"]) == Version(package["version"])]
            check(len(matched) == 1, "Selected file lacks exact original artifact mapping")
            item = matched[0]
            check("size" not in entry or (type(entry["size"]) is int and entry["size"] == item["size"]), "Selected file size differs")
            if item["eligible"]:
                matches.append(item)
        # Even inactive lock rows may not introduce sources outside the catalog.
        if not enabled:
            continue
        check(len(matches) == 1 and package["name"] not in selected, "Ambiguous/ineligible selected files")
        selected[package["name"]] = matches[0]
    direct = {}
    for row in projection["direct_roots"]:
        root = checked_input(row["requirement"])
        if catalog.active(root, target):
            name = canonicalize(root.name)
            check(name in selected and selected[name]["id"] == row["candidate_id"], "Selected direct root identity differs")
            direct[name] = row["candidate_id"]
    # No deterministic file ranking is adopted. An exact approved direct root is
    # already a qualified selection rule; ordinary same-version alternatives are not.
    for name, chosen in selected.items():
        if name not in direct:
            peers = [c for c in candidates if c["name"] == name and c["eligible"]
                     and Version(c["version"]) == Version(chosen["version"])]
            check(len(peers) == 1, "Ambiguous admitted same-version files")
    facts = {f["id"]: f for f in evidence["facts"]}
    reached = {}
    queue = [checked_input(raw) for raw in request["roots"]]
    count = 0
    while queue:
        count += 1
        catalog.bounded(count, catalog.MAX_EDGES * (catalog.MAX_PROJECTS + 1), "Selected closure work budget exceeded")
        req = queue.pop()
        if not catalog.active(req, target):
            continue
        name = canonicalize(req.name)
        check(name in selected, "Missing selected closure member")
        chosen = selected[name]
        check(req.url == chosen["url"] if req.url else req.specifier.contains(chosen["version"], prereleases=True),
              "Selected closure violates original requirement")
        for raw in request["constraints"]:
            restriction = checked_input(raw)
            if canonicalize(restriction.name) == name and catalog.active(restriction, target):
                check(restriction.specifier.contains(chosen["version"], prereleases=True), "Selected closure violates original constraint")
        incoming = {canonicalize(extra) for extra in req.extras}
        fact = facts[chosen["id"]]
        check(incoming <= set(fact["extras"]), "Selected extras undeclared")
        prior = reached.get(name)
        if prior is not None and incoming <= prior:
            continue
        reached.setdefault(name, set()).update(incoming)
        for raw in fact["dependencies"]:
            dependency = checked_input(raw)
            if catalog.active(dependency, target, {"", *reached[name]}):
                # Marker activation belongs to this parent extras context, not
                # a second evaluation against the child's empty extra.
                queue.append(requirement(str(dependency).split(";", 1)[0]))
    check(set(reached) == set(selected), "Unrelated selected packages")
    return {"schema": "pumas.selected-wheel-packet.v1", "request_sha256": digest(request),
            "catalog_sha256": digest(evidence), "target_observation_sha256": digest(observation),
            "projection_sha256": digest(projection),
            "selected": [{k: c[k] for k in (*evidence["candidates"][0].keys(), "local")} for _, c in sorted(selected.items())],
            "extras": {name: sorted(extras) for name, extras in sorted(reached.items())}}


def read(path, maximum):
    check(not path.is_symlink() and path.is_file(), "Missing/linked selection input")
    catalog.bounded(path.stat().st_size, maximum, "Selection input byte budget exceeded")
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    catalog.bounded(len(data), maximum, "Selection input byte budget exceeded")
    return data


def write(path, value):
    body = json.dumps(value, sort_keys=True, separators=(",", ":")).encode() + b"\n"
    catalog.bounded(len(body), catalog.MAX_OUTPUT, "Selection output byte budget exceeded")
    with path.open("xb") as stream:
        stream.write(body)


def main():
    parser = argparse.ArgumentParser()
    for name in ("request", "observation", "catalog", "wheels", "directory"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        request = catalog.decode(read(args.request, 10 * catalog.MAX_PAGE))
        observation = catalog.decode(read(args.observation, 64 * 1024))
        evidence = catalog.decode(read(args.catalog, catalog.MAX_OUTPUT))
        directory = args.directory
        check(not directory.is_symlink() and directory.is_dir(), "Missing owned selection directory")
        projection = prepare(request, observation, evidence, args.wheels.resolve())
        roots = ("\n".join(projection["roots"]) + "\n").encode()
        constraints = ("\n".join(projection["constraints"]) + "\n").encode()
        if not args.check:
            for path, body in ((directory / "roots.in", roots), (directory / "constraints.in", constraints)):
                with path.open("xb") as stream:
                    stream.write(body)
            write(directory / "projection.json", projection)
        else:
            check(catalog.decode(read(directory / "projection.json", catalog.MAX_OUTPUT)) == projection
                  and read(directory / "roots.in", catalog.MAX_OUTPUT) == roots
                  and read(directory / "constraints.in", catalog.MAX_OUTPUT) == constraints,
                  "Owned solver input projection changed")
            body = read(directory / "pylock.toml", catalog.MAX_OUTPUT)
            packet = selected_packet(request, observation, evidence, projection,
                                     tomllib.loads(body.decode("utf-8")), directory)
            packet["lock_sha256"] = catalog.hashlib.sha256(body).hexdigest()
            catalog.checked_target(request, observation)
            write(directory / "selected.json", packet)
    except catalog.Incomplete:
        print("Offline wheel selection incomplete", file=sys.stderr)
        raise SystemExit(20) from None
    except (ValueError, TypeError, KeyError, OSError, zipfile.BadZipFile, ExceptionGroup, tokenize.TokenError):
        print("Offline wheel selection refused", file=sys.stderr)
        raise SystemExit(21) from None


if __name__ == "__main__":
    main()
