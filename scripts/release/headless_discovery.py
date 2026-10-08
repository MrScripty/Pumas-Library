"""Fixed PumasBuildInfo/HttpServiceDescription v1 packaging boundary.

The Rust observer authenticates the existing core owner and its HTTP generation.
These are point-in-time observations, never a lifetime lease or model readiness.
"""

import ipaddress
import json
import re
import subprocess
import tempfile
import urllib.parse
import urllib.request
from pathlib import Path

HTTP_DISCOVERY_PATH = "/.well-known/pumas"
MAX_DESCRIPTION = 64 * 1024
BUILD_FIELDS = {
    "build_info_schema_version",
    "component",
    "package_version",
    "build_id",
    "source_revision",
    "target",
    "compiled_features",
    "protocols",
    "schemas",
}
INSTANCE_FIELDS = {
    "discovery_schema_version",
    "build_info",
    "registry_library_id",
    "library_root",
    "generation",
    "pumas_version",
    "protocols",
    "capabilities",
    "model_ref_schema_version",
    "selector_schema_version",
}
SERVICE_FIELDS = {
    "advertisement_schema_version",
    "service_generation",
    "instance",
    "endpoint",
    "build_info",
}
CORE_SCHEMAS = {
    "pumas.build-info": 1,
    "pumas.discovery": 1,
    "pumas.model-ref": 1,
    "pumas.model-selector": 1,
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fields(value, names, label):
    require(type(value) is dict and set(value) == names, f"invalid {label} fields")


def equivalent(left, right):
    # Python equality equates True and 1; the wire contract does not.
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def token(value):
    return isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_./+-]{0,127}", value)


def labels(value):
    return (
        type(value) is list
        and len(value) <= 32
        and all(token(item) for item in value)
        and len(set(value)) == len(value)
    )


def advertisements(value, versions):
    require(type(value) is list and 0 < len(value) <= 32, "invalid advertisements")
    observed = {}
    for item in value:
        fields(item, {"name", "versions" if versions else "version"}, "advertisement")
        require(token(item["name"]) and item["name"] not in observed, "invalid advertisement name")
        numbers = item["versions"] if versions else [item["version"]]
        require(
            type(numbers) is list
            and 0 < len(numbers) <= 32
            and all(type(number) is int and 0 < number < 2**31 for number in numbers)
            and len(set(numbers)) == len(numbers),
            "invalid advertisement version",
        )
        observed[item["name"]] = numbers if versions else numbers[0]
    return observed


def validate_build_info(info, component):
    # Pinned distribution inputs require provenance even though the shared wire
    # type deliberately allows it to be absent in ordinary development builds.
    fields(info, BUILD_FIELDS, "PumasBuildInfo")
    require(
        type(info["build_info_schema_version"]) is int
        and info["build_info_schema_version"] == 1
        and info["component"] == component,
        "incompatible PumasBuildInfo schema/component",
    )
    require(
        all(token(info[name]) for name in ("package_version", "build_id", "target"))
        and isinstance(info["source_revision"], str)
        and re.fullmatch(r"[a-f0-9]{40}", info["source_revision"]),
        "missing build provenance",
    )
    require(labels(info["compiled_features"]), "invalid compiled features")
    protocols = advertisements(info["protocols"], True)
    schemas = advertisements(info["schemas"], False)
    required = dict(CORE_SCHEMAS)
    require(1 in protocols.get("pumas.local-ipc", []), "incompatible local IPC protocol")
    if component == "pumas-rpc":
        required["pumas.http-advertisement"] = 1
        require(1 in protocols.get("pumas.local-http", []), "incompatible local HTTP protocol")
        require(
            {"pumas-rpc/s3", "pumas-rpc/inference-plugins"} <= set(info["compiled_features"]),
            "required RPC compile features absent",
        )
    require(
        all(schemas.get(name) == version for name, version in required.items()),
        "incompatible shared schema advertisement",
    )
    require(
        {"pumas-library/s3", "pumas-library/onnx-runtime"} <= set(info["compiled_features"]),
        "required core compile features absent",
    )
    require(
        not any(name.endswith("/test-support") for name in info["compiled_features"]),
        "test-support build is not a distribution candidate",
    )


def validate_pins(expected, target=None):
    for name, component in (("build_info", "pumas-rpc"), ("core_build_info", "pumas-library")):
        info = expected[name]
        validate_build_info(info, component)
        require(
            info["package_version"] == expected["version"]
            and info["build_id"] == expected["build_id"]
            and info["source_revision"] == expected["source_commit"],
            "build/cohort identity mismatch",
        )
        if target is not None:
            require(info["target"] == target, "build target mismatch")
    rpc, core = expected["build_info"], expected["core_build_info"]
    require(
        rpc["target"] == core["target"]
        and [name for name in rpc["compiled_features"] if name.startswith("pumas-library/")]
        == core["compiled_features"],
        "RPC/core build context differs",
    )


def endpoint(value):
    require(isinstance(value, str) and len(value) <= 128, "invalid local HTTP endpoint")
    try:
        url = urllib.parse.urlsplit(value)
        address = ipaddress.ip_address(url.hostname or "")
        port = 80 if url.port is None else url.port
    except ValueError as error:
        raise ValueError("invalid local HTTP endpoint") from error
    require(
        url.scheme == "http"
        and address.is_loopback
        and getattr(address, "scope_id", None) is None
        and value == value.strip()
        and not any(ord(char) < 32 for char in value)
        and url.username is None
        and url.password is None
        and not url.query
        and not url.fragment
        and url.path in ("", "/")
        and 0 < port < 65536
        and not value.endswith(":"),
        "numeric loopback base URL required",
    )
    return value.rstrip("/")


def identity(value):
    return isinstance(value, str) and 0 < len(value) <= 128 and not any(ord(c) < 32 for c in value)


def validate_description(description, expected, root, expected_endpoint=None):
    fields(description, SERVICE_FIELDS, "HttpServiceDescription")
    require(
        type(description["advertisement_schema_version"]) is int
        and description["advertisement_schema_version"] == 1
        and identity(description["service_generation"]),
        "missing/incompatible HTTP owner identity",
    )
    observed_endpoint = endpoint(description["endpoint"])
    require(
        expected_endpoint is None or observed_endpoint == endpoint(expected_endpoint),
        "HTTP endpoint differs from launched child",
    )
    instance = description["instance"]
    fields(instance, INSTANCE_FIELDS, "InstanceDescription")
    require(
        all(
            type(instance[name]) is int and instance[name] == 1
            for name in (
                "discovery_schema_version",
                "model_ref_schema_version",
                "selector_schema_version",
            )
        ),
        "incompatible owner schema",
    )
    require(
        identity(instance["registry_library_id"])
        and identity(instance["generation"])
        and instance["library_root"] == str(Path(root).resolve(strict=True)),
        "missing or changed library owner context",
    )
    protocols = advertisements(instance["protocols"], True)
    require(1 in protocols.get("pumas.local-ipc", []), "incompatible owner protocol")
    require(
        type(instance["capabilities"]) is list
        and all(isinstance(item, str) for item in instance["capabilities"])
        and {"model.query@1", "model.get.local@1", "model.selector@1", "artifact.resolve@1"}
        <= set(instance["capabilities"]),
        "local-first owner capabilities absent",
    )
    require(
        equivalent(description["build_info"], expected["build_info"])
        and equivalent(instance["build_info"], expected["core_build_info"])
        and instance["pumas_version"] == expected["core_build_info"]["package_version"],
        "live PumasBuildInfo mismatch",
    )
    return observed_endpoint


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        raise ValueError("handshake redirects refused")


def decode(data):
    require(len(data) <= MAX_DESCRIPTION, "description exceeds observation limit")

    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate description key")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError("nonfinite description number")

    return json.loads(data, object_pairs_hook=unique, parse_constant=invalid_constant)


def observe(binary, root, environment, timeout):
    # Output is disk-backed and bounded on read; a peer cannot make communicate()
    # accumulate an unbounded response. This exact helper never owns the service.
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        result = subprocess.run(
            [str(binary), "--describe-local-http", "--launcher-root", str(root)],
            env=environment,
            stdout=output,
            stderr=errors,
            timeout=timeout,
            check=False,
        )
        require(result.returncode == 0, "authenticated local owner observation refused")
        output.seek(0)
        return decode(output.read(MAX_DESCRIPTION + 1))


def verify_owner(binary, root, environment, expected, timeout=10, expected_endpoint=None):
    before = observe(binary, root, environment, timeout)
    url = validate_description(before, expected, root, expected_endpoint)
    http = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with http.open(url + HTTP_DISCOVERY_PATH, timeout=timeout) as response:
        require(response.status == 200, "owner handshake route failed")
        live = decode(response.read(MAX_DESCRIPTION + 1))
    validate_description(live, expected, root, expected_endpoint)
    require(equivalent(live, before), "HTTP owner generation/context changed")
    after = observe(binary, root, environment, timeout)
    require(equivalent(before, after), "authenticated owner changed during observation")
    return before
