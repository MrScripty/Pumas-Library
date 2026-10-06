"""Explicit wheel target data. Construction never discovers the inspection host.

Native observation is a separate operation, run only in the selected interpreter.
This contract is compatibility data, not source or execution authority.
"""
import importlib
import hashlib
import json
from pathlib import Path
import re
from types import MappingProxyType

tooling = Path(__file__).with_name("packaging-tooling.zip")
if not tooling.is_file():
    tooling = Path(__file__).parent / "tooling/packaging.zip"
if tooling.is_file():
    import sys
    sys.path.insert(0, str(tooling))
tags_api = importlib.import_module("packaging.tags")
markers_api = importlib.import_module("packaging.markers")
SpecifierSet = importlib.import_module("packaging.specifiers").SpecifierSet

MARKER_KEYS = frozenset({"implementation_name", "implementation_version", "os_name",
    "platform_machine", "platform_python_implementation", "platform_release", "platform_system",
    "platform_version", "python_full_version", "python_version", "sys_platform"})
FIELDS = frozenset({"schema", "python", "abi", "os", "arch", "libc", "macos_deployment",
                    "native_linux_tag", "markers"})


class UnsupportedTarget(ValueError):
    pass


def require(value, reason):
    if not value:
        raise UnsupportedTarget(reason)


def version_pair(raw):
    require(isinstance(raw, str) and re.fullmatch(r"[1-9]\d?\.(?:0|[1-9]\d?)", raw),
            "Target platform version must be an explicit bounded major.minor")
    return tuple(map(int, raw.split(".")))


class WheelTarget:
    def __init__(self, document):
        require(type(document) is dict and document.keys() == FIELDS, "Incomplete/unknown wheel target fields")
        require(document["schema"] == "pumas.wheel-target.v1", "Unsupported wheel target schema")
        require(isinstance(document["os"], str) and isinstance(document["arch"], str),
                "Invalid OS/architecture target fields")
        python = document["python"]
        require(isinstance(python, str) and len(python) <= 32 and re.fullmatch(r"3\.(?:1\d|[2-9]\d)\.(?:0|[1-9]\d*)", python),
                "Target requires explicit stable CPython3.10+ major.minor.patch")
        release = tuple(map(int, python.split(".")))
        abi = "cp" + str(release[0]) + str(release[1])
        require(document["abi"] == abi, "Unsupported target ABI: normal GIL CPython ABI required")
        require((document["os"], document["arch"]) in {("linux", "x86_64"), ("windows", "x86_64"), ("macos", "arm64")},
                "Unsupported OS/architecture wheel target")
        require(type(document["native_linux_tag"]) is bool, "Explicit native Linux tag policy required")
        environment = document["markers"]
        require(type(environment) is dict and environment.keys() == MARKER_KEYS, "Complete target marker environment required")
        require(all(isinstance(v, str) and 0 < len(v) <= 256 and not any(ord(c) < 32 for c in v)
                    for v in environment.values()), "Invalid target marker values")
        expected = {"implementation_name": "cpython", "implementation_version": python,
                    "platform_python_implementation": "CPython", "python_full_version": python,
                    "python_version": ".".join(python.split(".")[:2])}
        system, os_name, platform_system, machines = {
            "linux": ("linux", "posix", "Linux", {"x86_64"}),
            "windows": ("win32", "nt", "Windows", {"AMD64", "x86_64"}),
            "macos": ("darwin", "posix", "Darwin", {"arm64"}),
        }[document["os"]]
        expected.update(sys_platform=system, os_name=os_name, platform_system=platform_system)
        require(all(environment[k] == v for k, v in expected.items())
                and environment["platform_machine"] in machines, "Contradictory target marker identity")
        platforms = []
        if document["os"] == "linux":
            libc = document["libc"]
            require(type(libc) is dict and libc.keys() == {"family", "version"}, "Explicit Linux libc policy required")
            major, minor = version_pair(libc["version"])
            require(document["macos_deployment"] is None, "Contradictory Linux deployment policy")
            if libc["family"] == "glibc":
                require(major == 2 and minor >= 5, "Unsupported glibc policy")
                for floor in range(minor, 4, -1):
                    platforms.append(f"manylinux_2_{floor}_x86_64")
                    if floor in {17, 12, 5}:
                        platforms.append({17: "manylinux2014_x86_64", 12: "manylinux2010_x86_64", 5: "manylinux1_x86_64"}[floor])
            elif libc["family"] == "musl":
                require(major == 1, "Unsupported musl policy")
                platforms.extend(f"musllinux_1_{floor}_x86_64" for floor in range(minor, -1, -1))
            else:
                raise UnsupportedTarget("Unsupported/unknown Linux libc family")
            if document["native_linux_tag"]:
                platforms.append("linux_x86_64")
        else:
            require(document["libc"] is None and not document["native_linux_tag"], "Contradictory non-Linux libc policy")
            if document["os"] == "windows":
                require(document["macos_deployment"] is None, "Contradictory Windows deployment policy")
                platforms.append("win_amd64")
            else:
                deployment = version_pair(document["macos_deployment"])
                require(deployment[0] >= 11, "Unsupported macOS arm64 deployment policy")
                platforms.extend(tags_api.mac_platforms(version=deployment, arch="arm64"))
        self.python = python
        self.abi = abi
        self.system = document["os"]
        self.platforms = tuple(platforms)
        self.markers = MappingProxyType(dict(environment))
        self._wire = json.dumps(document, sort_keys=True, separators=(",", ":"))
        # Supplying version, ABI, interpreter and platforms avoids all packaging
        # interpreter/ABI/platform discovery defaults, including host GIL flags.
        self.tags = tuple(dict.fromkeys([
            *tags_api.cpython_tags(python_version=release[:2], abis=[abi], platforms=platforms),
            *tags_api.compatible_tags(python_version=release[:2], interpreter=abi, platforms=platforms),
        ]))

    def to_dict(self):
        return json.loads(self._wire)

    def marker_environment(self, extra=""):
        return {**self.markers, "extra": extra}

    def supports(self, tags):
        return bool(set(self.tags).intersection(tags))

    def allows_python(self, specification):
        return SpecifierSet(specification).contains(self.python, prereleases=True)

    def require_native_consumer(self):
        observed = capture_native()
        require(self.markers == observed.markers and self.abi == observed.abi
                and set(self.tags) <= set(observed.tags), "Explicit target differs from selected native consumer")


def capture_native():
    """Call inside the selected interpreter, never as cross-target inference."""
    import platform
    import sys
    import sysconfig
    require(not sysconfig.get_config_var("Py_DEBUG") and not sysconfig.get_config_var("Py_GIL_DISABLED"),
            "Debug/free-threaded native ABI is unsupported")
    actual_tags = set(tags_api.sys_tags())
    # Windows omits Py_DEBUG. Public interpreter tags account for its debug
    # refcount/suffix probes; debug builds may also accept a normal ABI.
    require(not any(re.fullmatch(r"cp\d+[td]+", tag.abi) for tag in actual_tags),
            "Debug/free-threaded native ABI is unsupported")
    markers = markers_api.default_environment()
    systems = {"linux": "linux", "win32": "windows", "darwin": "macos"}
    require(sys.platform in systems, "Unsupported native consumer platform")
    system = systems[sys.platform]
    machine = platform.machine()
    arch = "x86_64" if machine in {"x86_64", "AMD64"} else machine
    libc = None
    macos = None
    if system == "linux":
        platforms = {tag.platform for tag in actual_tags}
        for family, prefix in (("glibc", "manylinux"), ("musl", "musllinux")):
            versions = [tuple(map(int, match.groups())) for p in platforms
                        if (match := re.fullmatch(prefix + r"_(\d+)_(\d+)_x86_64", p))]
            if versions:
                require(libc is None, "Ambiguous native libc observation")
                libc = {"family": family, "version": ".".join(map(str, max(versions)))}
        require(libc is not None, "Native Linux libc policy cannot be established")
    elif system == "macos":
        macos = ".".join(platform.mac_ver()[0].split(".")[:2])
    observed = WheelTarget({"schema": "pumas.wheel-target.v1", "python": markers["python_full_version"],
        "abi": "cp" + markers["python_version"].replace(".", ""), "os": system, "arch": arch,
        "libc": libc, "macos_deployment": macos, "native_linux_tag": system == "linux", "markers": markers})
    require(set(observed.tags) <= actual_tags,
            "Native target differs from actual interpreter compatibility tags")
    return observed


def observation_digest(document):
    """Fixed JSON projection shared with the repository JSON serializer."""
    return hashlib.sha256(json.dumps(document, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode("utf-8")).hexdigest()


def checked_observation(document):
    require(isinstance(document, dict) and set(document) == {
        "schema", "interpreter", "interpreter_sha256", "target"}, "Incomplete target observation")
    require(document["schema"] == "pumas.wheel-target-observation.v1", "Unsupported target observation")
    interpreter = document["interpreter"]
    require(isinstance(interpreter, str) and 0 < len(interpreter) <= 4096
            and not any(ord(c) < 32 or ord(c) == 127 for c in interpreter), "Invalid observed interpreter")
    # The observer can describe any admitted platform; path identity remains
    # exact data rather than an inspection-host Path.is_absolute() decision.
    require(interpreter.startswith("/") or re.match(r"[A-Za-z]:[\\/]", interpreter)
            or interpreter.startswith("\\\\"), "Observed interpreter path is not absolute")
    require(isinstance(document["interpreter_sha256"], str)
            and re.fullmatch(r"[0-9a-f]{64}", document["interpreter_sha256"]), "Invalid observed interpreter digest")
    return WheelTarget(document["target"])


def capture_observation():
    """Trusted caller runs this in its selected interpreter under stage custody."""
    import sys
    document = {"schema": "pumas.wheel-target-observation.v1", "interpreter": sys.executable,
                "interpreter_sha256": interpreter_digest(sys.executable),
                "target": capture_native().to_dict()}
    checked_observation(document)
    return document


def interpreter_digest(path):
    hashed = hashlib.sha256()
    with open(path, "rb") as executable:
        for chunk in iter(lambda: executable.read(1024 * 1024), b""):
            hashed.update(chunk)
    return hashed.hexdigest()


def resolution_target(resolution, observation=None):
    """Require separate owner-approved evidence for target-bearing packets."""
    keys = {"wheel_target", "wheel_target_observation_sha256"}
    present = keys.intersection(resolution)
    if not present and observation is None:
        return None  # Explicit legacy mode, never an explicit-target fallback.
    require(present == keys and observation is not None, "Target packet lacks approved observation")
    target = checked_observation(observation)
    declared = WheelTarget(resolution["wheel_target"])
    require(declared.to_dict() == target.to_dict()
            and resolution["wheel_target_observation_sha256"] == observation_digest(observation),
            "Target packet differs from approved observation")
    require(resolution.get("interpreter") == observation["interpreter"]
            and resolution.get("python") == target.markers["python_version"]
            and resolution.get("implementation") == "cpython"
            and resolution.get("machine") == target.markers["platform_machine"],
            "Target packet interpreter identity differs from approval")
    return target


def bind_resolution(resolution, observation):
    """Add bindings only; original package/source/report identity stays intact."""
    target = checked_observation(observation)
    bound = {**resolution, "wheel_target": target.to_dict(),
             "wheel_target_observation_sha256": observation_digest(observation)}
    resolution_target(bound, observation)
    return bound


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--observe", action="store_true", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        document = json.dumps(capture_observation(), sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        if args.output is None:
            print(document)
        else:
            with args.output.open("x", encoding="utf-8") as destination:
                destination.write(document + "\n")
    except (OSError, ValueError):
        parser.exit(3, "Selected interpreter target observation refused\n")
