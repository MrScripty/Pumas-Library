#!/usr/bin/env python3
"""Callable CLI consumer reference, not a Python SDK or a new wire protocol.

Linux/Python 3.11+, a pinned pumas-rpc binary, and the same persisted registry.
See docs/contracts/local-http-consumer.md for ownership and qualification limits.
"""
import argparse
import asyncio
import copy
from contextlib import asynccontextmanager
import hashlib
import ipaddress
import json
import math
import os
from pathlib import Path
import re
import sys
import urllib.parse

MAX_DESCRIPTION = 64 * 1024
MAX_RPC = 1024 * 1024
MAX_OPERATION = 32 * 1024 * 1024
DESCRIPTOR_SCHEMA_SHA256 = "eb25783123d38de5ff6aedf4cfa69de8f4f5f5c60799768b1b90da258b85f922"

# Concrete representations from the pinned producer contract, not model or
# capability names. A pair describes formats to choose, never payload admission.
INPUT_MODALITIES = {
    "text": {"messages_text", "text", "text_batch"},
    "image": {"png_base64", "jpeg_base64", "messages_image"},
    "audio": {"pcm_s16le", "pcm_f32le"},
}
OUTPUT_MODALITIES = {
    "text": {"text"}, "embeddings": {"embeddings_float32"},
    "image": {"png_base64"}, "labels": {"labels"},
}


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _decode(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            _require(key not in result, "duplicate JSON field")
            result[key] = value
        return result

    def invalid(_):
        raise ValueError("nonfinite JSON value")

    def number(value):
        result = float(value)
        _require(math.isfinite(result), "nonfinite JSON value")
        return result

    return json.loads(data, object_pairs_hook=unique, parse_constant=invalid, parse_float=number)


def _advertisements(value, *, protocols):
    """Decode the existing named advertisements; never infer from SemVer/features."""
    _require(type(value) is list and 0 < len(value) <= 32, "invalid contract advertisements")
    observed = {}
    for item in value:
        field = "versions" if protocols else "version"
        _require(type(item) is dict and set(item) == {"name", field}, "invalid contract advertisement")
        name = item["name"]
        _require(isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_./+-]{0,127}", name)
                 and name not in observed, "ambiguous contract advertisement")
        numbers = item[field] if protocols else [item[field]]
        _require(type(numbers) is list and 0 < len(numbers) <= 32
                 and all(type(number) is int and 0 < number < 2**32 for number in numbers)
                 and len(set(numbers)) == len(numbers), "invalid contract version")
        observed[name] = numbers if protocols else numbers[0]
    return observed


def _descriptor_schema():
    path = Path(__file__).parent / "contracts" / "capability-descriptor.schema.json"
    data = path.read_bytes()
    _require(hashlib.sha256(data).hexdigest() == DESCRIPTOR_SCHEMA_SHA256,
             "pinned capability descriptor schema changed")
    return _decode(data)


def _descriptor_context(build_info, contract_version):
    # This only checks supplied observations, never authenticates a remote owner.
    _require(type(contract_version) is int and contract_version == 1,
             "unsupported descriptor contract version")
    _require(type(build_info) is dict and build_info.get("component") == "pumas-rpc"
             and type(build_info.get("build_info_schema_version")) is int
             and build_info["build_info_schema_version"] == 1, "unsupported producer build descriptor")
    protocols = _advertisements(build_info.get("protocols"), protocols=True)
    schemas = _advertisements(build_info.get("schemas"), protocols=False)
    _require(1 in protocols.get("pumas.local-http", []), "no common local HTTP protocol")
    _require(all(schemas.get(name) == 1 for name in (
        "pumas.http-advertisement", "pumas.http-admission-fence", "pumas.http-owner-retention",
        "pumas.model-operations.image-to-text")), "required descriptor/lifecycle contracts absent")


def _descriptor_value(value, schema):
    def object_fields(item, required):
        # The producer schema allows extra properties; retain every one.
        _require(type(item) is dict and set(required) <= item.keys(), "missing descriptor fields")

    def enum(item, name):
        _require(type(item) is str and item in schema["$defs"][name]["enum"],
                 "unknown descriptor discriminant")

    object_fields(value, schema["required"])
    enum(value["capability"], "Capability")
    enum(value["semantic_task"], "SemanticTask")
    for field, name in (("input_formats", "InputFormat"), ("output_formats", "OutputFormat")):
        _require(type(value[field]) is list, "descriptor formats must be arrays")
        for item in value[field]:
            enum(item, name)
    _require(type(value["streaming"]) is bool, "descriptor streaming must be Boolean")
    availability = value["availability"]
    object_fields(availability, ["state"])
    _require(type(availability["state"]) is str and availability["state"] in ("available", "unavailable"),
             "unknown descriptor availability")
    if availability["state"] == "unavailable":
        object_fields(availability, ["reason"])
        enum(availability["reason"], "AvailabilityReason")
    _require(type(value["option_bounds"]) is list, "descriptor bounds must be an array")
    for bound in value["option_bounds"]:
        object_fields(bound, schema["$defs"]["OptionBound"]["required"])
        enum(bound["option"], "OptionName")
        for field in ("minimum", "maximum"):
            _require(type(bound[field]) in (int, float), "descriptor bounds must be numbers")
            try:
                finite = math.isfinite(bound[field])
            except OverflowError:
                finite = False
            _require(finite, "descriptor bound cannot be represented as finite f64")
    return value


def decode_capability_descriptor(data, *, build_info, contract_version):
    """Decode pinned DTO bytes; supplied identity/availability is observational."""
    _descriptor_context(build_info, contract_version)
    _require(type(data) is bytes and len(data) <= MAX_RPC, "bounded descriptor JSON bytes required")
    value = _decode(data)
    return _descriptor_value(value, _descriptor_schema())


def select_capability(descriptors, *, build_info, contract_version, input_modality,
                      output_modality, semantic_task=None, stream=False):
    """Select declared formats without model/capability-name switches or dispatch."""
    _descriptor_context(build_info, contract_version)
    schema = _descriptor_schema()
    _require(type(descriptors) is list and len(descriptors) <= 32, "bounded descriptor observations required")
    _require(type(input_modality) is str and input_modality in INPUT_MODALITIES
             and type(output_modality) is str and output_modality in OUTPUT_MODALITIES,
             "unsupported generic modality")
    _require(type(stream) is bool, "explicit Boolean streaming requirement required")
    _require(semantic_task is None or (type(semantic_task) is str
             and semantic_task in schema["$defs"]["SemanticTask"]["enum"]), "unknown semantic task")
    # Clone through the bytes decoder so results do not alias caller data.
    # JSON compatibility and the size bound are host policies.
    encoded = json.dumps(descriptors, allow_nan=False).encode()
    _require(len(encoded) <= MAX_RPC, "descriptor observations exceed reference bound")
    values = _decode(encoded)
    names = set()
    candidates = []
    for value in values:
        _descriptor_value(value, schema)
        _require(value["capability"] not in names, "duplicate/contradictory capability observations")
        names.add(value["capability"])
        options = set()
        for bound in value["option_bounds"]:
            _require(bound["option"] not in options and bound["minimum"] <= bound["maximum"],
                     "duplicate or reversed option bounds")
            options.add(bound["option"])
        if (INPUT_MODALITIES[input_modality].intersection(value["input_formats"])
                and OUTPUT_MODALITIES[output_modality].intersection(value["output_formats"])
                and (semantic_task is None or semantic_task == value["semantic_task"])
                and (not stream or value["streaming"])):
            candidates.append(value)
    _require(candidates, "no declared modality pair")
    available = [value for value in candidates if value["availability"]["state"] == "available"]
    _require(available, "declared modality pair unavailable")
    _require(len(available) == 1, "ambiguous modality pair; choose a semantic task")
    return available[0]


def _selection(description, root):
    _require(type(description) is dict and type(description.get("advertisement_schema_version")) is int
             and description["advertisement_schema_version"] == 1,
             "unsupported HTTP description")
    instance = description["instance"]
    _require(type(instance["discovery_schema_version"]) is int
             and instance["discovery_schema_version"] == 1
             and Path(instance["library_root"]).is_dir()
             and Path(instance["library_root"]).samefile(root), "selected root differs")
    info = description["build_info"]
    _require(info["component"] == "pumas-rpc" and type(info["build_info_schema_version"]) is int
             and info["build_info_schema_version"] == 1, "unsupported HTTP build descriptor")
    protocols = _advertisements(info.get("protocols"), protocols=True)
    _require(1 in protocols.get("pumas.local-http", []), "no common local HTTP protocol")
    schemas = _advertisements(info.get("schemas"), protocols=False)
    _require(all(schemas.get(name) == 1 for name in
                 ("pumas.http-advertisement", "pumas.http-admission-fence", "pumas.http-owner-retention")),
             "required producer contracts absent")
    fence = (instance["generation"], description["service_generation"])
    _require(all(isinstance(value, str) and 0 < len(value) <= 256
                 and all(0x21 <= ord(char) <= 0x7e and char != ',' for char in value)
                 for value in fence), "invalid generation fence")
    url = urllib.parse.urlsplit(description["endpoint"])
    _require(url.scheme == "http" and ipaddress.ip_address(url.hostname).is_loopback
             and url.username is None and url.password is None and url.path in ("", "/")
             and not url.query and not url.fragment and url.port is not None
             and 0 < url.port < 65536 and '%' not in url.hostname, "numeric loopback endpoint required")
    return url, fence


async def _stop(process):
    if process.returncode is None:
        try:
            process.terminate()
        except ProcessLookupError:
            pass
        try:
            await asyncio.wait_for(process.wait(), 10)
        except TimeoutError:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            await asyncio.wait_for(process.wait(), 10)
            raise RuntimeError("consumer child required forced shutdown; owner custody unresolved")
    return await process.wait()


async def _settle(awaitable):
    """Observe a finite child creation/cleanup despite caller cancellation."""
    task = asyncio.create_task(awaitable)
    cancelled = False
    while not task.done():
        try:
            await asyncio.shield(task)
        except asyncio.CancelledError:
            cancelled = True
    result = task.result()
    if cancelled:
        raise asyncio.CancelledError
    return result


class _Child:
    """Supervise only children created here; continuously drain bounded log lines."""
    def __init__(self, process, prefix, validate):
        self.process, self.prefix, self.validate = process, prefix, validate
        self.first = asyncio.get_running_loop().create_future()
        self.last = None
        self.failure = None
        self.reader = asyncio.create_task(self._read())

    async def _read(self):
        try:
            while line := await self.process.stdout.readline():
                if not line.startswith(self.prefix):
                    continue
                value = _decode(line[len(self.prefix):])
                self.validate(value)
                if self.last is not None:
                    _require(self.prefix == b"PUMAS_LOCAL_RETENTION="
                             and self.last["state"] == "retained" and value["state"] == "revoked",
                             "unexpected repeated CLI acknowledgment")
                elif self.prefix == b"PUMAS_LOCAL_RETENTION=":
                    _require(value["state"] == "retained", "retention revoked before acknowledgment")
                self.last = value
                if not self.first.done():
                    self.first.set_result(value)
        except Exception as error:
            self._failed(error)
            # An invalid/oversized line must not strand a live child's stdout.
            # After first failure discard bounded chunks rather than parsing.
            try:
                while await self.process.stdout.read(4096):
                    pass
            except Exception as drain_error:
                self._failed(drain_error)
        try:
            code = await self.process.wait()
            _require(code == 0, "CLI child failed; owner availability unconfirmed")
            _require(self.last is not None, "CLI child ended before acknowledgment")
        except Exception as error:
            self._failed(error)

    def _failed(self, error):
        if self.failure is None:
            self.failure = error
        if not self.first.done():
            self.first.set_exception(self.failure)

    async def ready(self):
        return await asyncio.wait_for(asyncio.shield(self.first), 20)

    async def close(self):
        try:
            code = await _stop(self.process)
            await self.reader
            if self.failure is not None:
                raise RuntimeError("CLI acknowledgment/log reader failed; custody unconfirmed") from self.failure
            if code != 0:
                raise RuntimeError("CLI child failed; owner custody unconfirmed")
            return code
        finally:
            if self.process.returncode is None:
                # Failure to observe even forced child exit remains an error;
                # do not wait forever on its still-open pipe or claim cleanup.
                self.reader.cancel()
            try:
                await self.reader
            except asyncio.CancelledError:
                pass
            if self.first.done() and not self.first.cancelled():
                self.first.exception()  # Observe an unsuccessful startup, including cancellation.


class LocalHttpSession:
    """A live CLI-owned passive guard and its authenticated HTTP selection."""
    def __init__(self, binary, root, environment):
        self.binary, self.root, self.environment = binary, root, environment
        self._description = None
        self._children = []
        self._bootstrap = None
        self._retention = None
        self._closed = False
        self._request_id = 0
        self.cleanup_results = []

    @property
    def description(self):
        return copy.deepcopy(self._description)

    @property
    def ownership(self):
        return "owned" if self._bootstrap and self._bootstrap.last["ownership"] == "owned" else "borrowed"

    @property
    def child_pids(self):
        return [child.process.pid for child in self._children]

    async def _create_child(self, mode, prefix, validate):
        process = await asyncio.create_subprocess_exec(
            str(self.binary), mode, "--launcher-root", str(self.root),
            *(["--port", "0"] if mode == "--attach-or-start-local-http" else []),
            env=self.environment, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL, limit=MAX_DESCRIPTION)
        child = _Child(process, prefix, validate)
        self._children.append(child)
        return child

    async def _spawn(self, mode, prefix, validate):
        return await _settle(self._create_child(mode, prefix, validate))

    async def _observe(self):
        process = await asyncio.create_subprocess_exec(
            str(self.binary), "--describe-local-http", "--launcher-root", str(self.root),
            env=self.environment, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        try:
            async with asyncio.timeout(10):
                data = bytearray()
                while chunk := await process.stdout.read(4096):
                    data.extend(chunk)
                    _require(len(data) <= MAX_DESCRIPTION, "HTTP description exceeds bound")
                _require(await process.wait() == 0, "authenticated owner selection refused")
                return _decode(data)
        finally:
            await _stop(process)

    async def _describe(self):
        return await _settle(self._observe())

    async def _open(self, allow_start):
        if allow_start:
            def bootstrap(value):
                _require(type(value) is dict and set(value) ==
                         {"bootstrap_schema_version", "ownership", "description"}
                         and type(value["bootstrap_schema_version"]) is int
                         and value["bootstrap_schema_version"] == 1
                         and value["ownership"] in ("owned", "borrowed"), "invalid bootstrap acknowledgment")
                _selection(value["description"], self.root)
            self._bootstrap = await self._spawn(
                "--attach-or-start-local-http", b"PUMAS_LOCAL_ACCESS=", bootstrap)
            acknowledgment = await self._bootstrap.ready()
            if acknowledgment["ownership"] == "borrowed":
                _require(await asyncio.wait_for(self._bootstrap.process.wait(), 10) == 0,
                         "borrower did not finish read-only selection")
        self._description = await self._describe()
        self._url, self._fence = _selection(self._description, self.root)
        if self._bootstrap:
            _require(self._bootstrap.last["description"] == self._description,
                     "bootstrap/authenticated owner changed")

        def retained(value):
            _require(type(value) is dict and set(value) == {"retention_schema_version", "state",
                     "instance_generation", "service_generation"}
                     and type(value["retention_schema_version"]) is int and value["retention_schema_version"] == 1
                     and value["state"] in ("retained", "revoked")
                     and (value["instance_generation"], value["service_generation"]) == self._fence,
                     "retention owner/schema changed")
        self._retention = await self._spawn("--retain-local-http-owner", b"PUMAS_LOCAL_RETENTION=", retained)
        await self._retention.ready()
        self._check_available()

    def _check_available(self):
        if self._closed or self._retention is None or self._retention.failure \
                or self._retention.process.returncode is not None \
                or self._retention.last["state"] != "retained" \
                or (self.ownership == "owned" and (self._bootstrap.failure
                                                  or self._bootstrap.process.returncode is not None)):
            raise RuntimeError("retention/owner availability unconfirmed; authenticate a fresh context")

    async def rpc(self, method, params=None, *, timeout=10):
        """One bounded JSON-RPC call. Cancellation closes transport; never retries."""
        self._check_available()
        _require(isinstance(method, str) and method, "RPC method required")
        _require(params is None or type(params) is dict, "object RPC params required")
        self._request_id += 1
        request_id = self._request_id
        message = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            message["params"] = params
        body = json.dumps(message, allow_nan=False).encode()
        status, result = await self._json_http("POST", "/rpc", body, timeout=timeout)
        _require(status == 200, "HTTP admission/transport failed; request outcome unconfirmed")
        _require(type(result) is dict and result.get("jsonrpc") == "2.0"
                 and type(result.get("id")) is int and result["id"] == request_id
                 and ("result" in result) != ("error" in result), "invalid RPC response identity")
        return result

    async def capabilities(self, model, *, profile=None):
        """Read selected-model declarations; catalog presence is not readiness."""
        _require(isinstance(model, str) and model, "selected model required")
        _require(profile is None or isinstance(profile, str), "profile must be a string")
        query = {"model": model}
        if profile is not None:
            query["profile"] = profile
        status, result = await self._json_http("GET", "/v1/capabilities?" + urllib.parse.urlencode(query))
        if status == 200:
            versions = result.get("supported_contract_versions")
            _require(type(versions) is list and all(type(version) is int for version in versions)
                     and 1 in versions and type(result.get("capabilities")) is list,
                     "no common model-operation contract")
        return {"status": status, "body": result}

    async def model_operation(self, request):
        """One finite modality-first operation; cancellation never authorizes replay."""
        _require(type(request) is dict and "capability" not in request
                 and type(request.get("contract_version")) is int and request["contract_version"] == 1
                 and isinstance(request.get("request_id"), str) and request["request_id"]
                 and type(request.get("stream", False)) is bool and not request.get("stream", False),
                 "finite modality-first contract v1 request required")
        request_id = request["request_id"]
        body = json.dumps(request, allow_nan=False).encode()
        status, result = await self._json_http("POST", "/v1/model-operations", body,
                                               timeout=None, limit=MAX_OPERATION)
        if "contract_version" in result:
            _require(type(result["contract_version"]) is int and result["contract_version"] == 1,
                     "unsupported operation response contract")
        if result.get("request_id") is not None:
            _require(result["request_id"] == request_id, "operation response identity changed")
        if status == 200:
            _require(result.get("contract_version") == 1 and result.get("request_id") == request_id
                     and type(result.get("result")) is dict and "error" not in result,
                     "invalid operation response identity")
        return {"status": status, "body": result}

    async def _json_http(self, method, target, body=b"", *, timeout=10, limit=MAX_RPC):
        self._check_available()
        _require(len(body) <= limit, "HTTP request exceeds reference limit")
        writer = None
        try:
            async with asyncio.timeout(timeout):
                reader, writer = await asyncio.wait_for(
                    asyncio.open_connection(self._url.hostname, self._url.port, limit=16384), 10)
                self._check_available()
                headers = (f"{method} {target} HTTP/1.1\r\nHost: {self._url.netloc}\r\n"
                           f"Pumas-Instance-Generation: {self._fence[0]}\r\n"
                           f"Pumas-Service-Generation: {self._fence[1]}\r\n"
                           f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
                           "Connection: close\r\n\r\n").encode("ascii")
                writer.write(headers + body)
                await writer.drain()
                head = await reader.readuntil(b"\r\n\r\n")
                lines = head.decode("ascii").split("\r\n")
                status = lines[0].split()
                _require(len(status) >= 2 and status[0] == "HTTP/1.1"
                         and re.fullmatch(r"[1-5][0-9]{2}", status[1]), "invalid HTTP response status")
                lengths = [line.split(":", 1)[1].strip() for line in lines[1:] if line.lower().startswith("content-length:")]
                _require(len(lengths) == 1 and re.fullmatch(r"[0-9]+", lengths[0])
                         and int(lengths[0]) <= limit
                         and not any(line.lower().startswith("transfer-encoding:") for line in lines[1:]),
                         "unsupported or oversized reference response framing")
                result = _decode(await reader.readexactly(int(lengths[0])))
                _require(type(result) is dict, "object HTTP response required")
                return int(status[1]), result
        finally:
            if writer is not None:
                writer.close()
                try:
                    await writer.wait_closed()
                except ConnectionError:
                    pass  # Local disposal still occurred; preserve caller cancellation/result.

    async def _close(self):
        self._closed = True
        errors = []
        for child in reversed(self._children):
            try:
                code = await child.close()
                self.cleanup_results.append({"pid": child.process.pid, "exit_code": code})
            except Exception as error:
                errors.append(error)
        if errors:
            raise ExceptionGroup("consumer cleanup incomplete; owner custody unconfirmed", errors)


async def _finish_cleanup(session):
    # A cancelled context still observes only its own helper/owner child cleanup.
    await _settle(session._close())


@asynccontextmanager
async def local_http_session(binary, root, *, binary_sha256, allow_start=False, environment=None):
    """Explicit start opt-in, authenticated selection, held guard and owned cleanup."""
    _require(sys.platform == "linux", "CLI consumer reference is qualified only on Linux")
    _require(type(allow_start) is bool, "explicit Boolean start opt-in required")
    binary, root = Path(binary).resolve(strict=True), Path(root).resolve(strict=True)
    _require(root.is_dir() and isinstance(binary_sha256, str) and re.fullmatch(r"[a-f0-9]{64}", binary_sha256),
             "existing root and exact binary SHA256 required")
    with binary.open("rb") as source:
        _require(hashlib.file_digest(source, "sha256").hexdigest() == binary_sha256, "pinned binary bytes changed")
    session = LocalHttpSession(binary, root, dict(os.environ if environment is None else environment))
    try:
        await session._open(allow_start)
        yield session
    finally:
        await _finish_cleanup(session)


async def _main(args):
    async with local_http_session(args.binary, args.root, binary_sha256=args.binary_sha256,
                                  allow_start=args.allow_start) as session:
        print(json.dumps({"ownership": session.ownership, "description": session.description}))
        print(json.dumps(await session.rpc("get_models")))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("root", type=Path)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--allow-start", action="store_true")
    asyncio.run(_main(parser.parse_args()))
