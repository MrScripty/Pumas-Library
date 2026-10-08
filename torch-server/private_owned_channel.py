"""Inherited, bounded private custody channel; no listener or public routes.

An inherited descriptor binds the owning parent; frame contents do not qualify
runtime code or mint artifact custody. Only the retained internal native gate
may prepare a load plan. The shipping gate refuses. EOF/partial frames close
admission and request cancellation, never claim native or child-tree cessation.
"""

import asyncio
import json
import socket
import struct
import sys

from owned_model_operations import OwnedModelOperations

MAX_REQUEST_BYTES = 32 * 1024 * 1024
MAX_REPLY_BYTES = 64 * 1024
MAX_PENDING = 64
OPERATIONS = frozenset(
    {"hello", "load", "use", "status", "cancel", "cancel_exchange", "unload", "close"}
)


class PrivateChannelError(ValueError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


class _PipeWriterProtocol(asyncio.streams.FlowControlMixin):
    def __init__(self, loop):
        super().__init__(loop=loop)
        self._closed = loop.create_future()

    def connection_lost(self, error):
        super().connection_lost(error)
        if not self._closed.done():
            if error is None:
                self._closed.set_result(None)
            else:
                self._closed.set_exception(error)

    def _get_close_waiter(self, stream):
        return self._closed


def _unique(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise PrivateChannelError("invalid_envelope")
        value[key] = item
    return value


def _envelope(raw):
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique)
    except (UnicodeError, ValueError, RecursionError):
        raise PrivateChannelError("invalid_envelope") from None
    if (
        type(value) is not dict
        or value.keys() != {"version", "exchange_id", "operation", "payload"}
        or type(value["version"]) is not int
        or value["version"] != 1
        or type(value["exchange_id"]) is not int
        or not 0 < value["exchange_id"] <= (1 << 64) - 1
        or type(value["operation"]) is not str
        or value["operation"] not in OPERATIONS
        or type(value["payload"]) is not dict
    ):
        raise PrivateChannelError("invalid_envelope")
    return value


def _fields(value, fields):
    if type(value) is not dict or value.keys() != fields:
        raise PrivateChannelError("invalid_request")


def _slot(ref):
    return {"slot_id": ref.slot_id, "load_generation": ref.load_generation}


def _load_status(status):
    return {
        "runtime_instance_id": status.slot_ref.runtime_instance_id,
        "slot": _slot(status.slot_ref),
        "state": status.state,
        "cleanup": status.cleanup,
        "error_code": status.error_code,
    }


def _operation_status(status):
    ref = status.operation_ref
    code = status.cleanup_diagnostic or status.operation_diagnostic
    result = {
        "runtime_instance_id": ref.runtime_instance_id,
        "slot": _slot(ref.slot_ref),
        "operation_id": ref.operation_id,
        "state": status.state,
        "cleanup": status.cleanup,
        "text": status.text,
        "finish_reason": status.finish_reason,
        "error_code": code.code if code is not None else None,
    }
    if status.state == "completed" and status.finish_reason not in ("stop", "length"):
        result.update(
            state="failed", text=None, finish_reason=None, error_code="finish_reason_unqualified"
        )
    return result


class PrivateOwnedChannel:
    """One reader, bounded retained handlers, serialized complete replies.

    status waiters do not block finite cancellation commands. No generation
    deadline, replay or polling is used. Duplicate/backwards exchange IDs and
    incoherent frames fence the channel. Pending decoded request bytes are also
    capped; a peer that stops consuming replies may retain bounded handlers.
    """

    def __init__(self, actor, *, native_gate=None, adapter=None):
        self.actor = actor
        self._gate = native_gate
        self._adapter = adapter
        self._writer_lock = asyncio.Lock()
        self._tasks = set()
        self._pending_bytes = 0
        self._last_exchange = 0
        self._hello = False
        self._closed = False
        self._bridge = None
        self._handles = {}
        self._waiters = set()
        self._writer = None
        self._records = {}
        self._read_transport = None

    async def serve_stdio(self, *, output=None):
        """Own the exact inherited stdin/stdout pair; stdout contains frames only."""
        loop = asyncio.get_running_loop()
        reader = asyncio.StreamReader(limit=MAX_REQUEST_BYTES + 4)
        self._read_transport, _ = await loop.connect_read_pipe(
            lambda: asyncio.StreamReaderProtocol(reader),
            sys.stdin.buffer,
        )
        protocol = _PipeWriterProtocol(loop)
        transport, _ = await loop.connect_write_pipe(
            lambda: protocol, sys.stdout.buffer if output is None else output
        )
        writer = asyncio.StreamWriter(transport, protocol, None, loop)
        try:
            await self.serve(reader, writer)
        finally:
            self._read_transport.close()

    async def serve_fd(self, fd):
        # The caller transfers this exact inherited descriptor; no path lookup,
        # bind, listener discovery or public client can provide the connection.
        reader, writer = await asyncio.open_connection(sock=socket.socket(fileno=fd))
        await self.serve(reader, writer)

    async def serve(self, reader, writer):
        self._writer = writer
        try:
            while not self._closed:
                header = await reader.readexactly(4)
                size = struct.unpack("!I", header)[0]
                if not 0 < size <= MAX_REQUEST_BYTES:
                    raise PrivateChannelError("request_limit")
                request = _envelope(await reader.readexactly(size))
                exchange = request["exchange_id"]
                if exchange <= self._last_exchange:
                    raise PrivateChannelError("invalid_exchange")
                self._last_exchange = exchange
                if (
                    len(self._tasks) >= MAX_PENDING
                    or self._pending_bytes + size > MAX_REQUEST_BYTES
                ):
                    await self._reply(
                        writer, request, error={"code": "runtime_busy", "effect": "not_admitted"}
                    )
                    continue
                self._pending_bytes += size
                record = {"request": request, "cancel": False, "load_ref": None}
                self._records[exchange] = record
                task = asyncio.create_task(self._handle(writer, request, record))
                self._tasks.add(task)
                task.add_done_callback(
                    lambda done, count=size, ident=exchange: self._done(done, count, ident)
                )
        except (asyncio.IncompleteReadError, PrivateChannelError, ConnectionError, OSError):
            pass
        finally:
            self._closed = True
            self.actor.close_admission()
            writer.close()
            # Retained actor tasks remain independent. Handler loss cannot
            # discharge their native/device/artifact custody.
            if self._tasks:
                await asyncio.gather(*self._tasks, return_exceptions=True)
            try:
                await writer.wait_closed()
            except (ConnectionError, OSError):
                pass

    def _done(self, task, count, exchange):
        self._tasks.discard(task)
        self._pending_bytes -= count
        self._records.pop(exchange, None)
        if not task.cancelled() and task.exception() is not None:
            self._closed = True
            self.actor.close_admission()
            self._writer.close()
            if self._read_transport is not None:
                self._read_transport.close()

    async def _reply(self, writer, request, *, result=None, error=None):
        reply = {key: request[key] for key in ("version", "exchange_id", "operation")}
        reply["error" if error is not None else "result"] = error if error is not None else result
        raw = json.dumps(reply, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(raw) > MAX_REPLY_BYTES:
            raise PrivateChannelError("response_limit")
        async with self._writer_lock:
            writer.write(struct.pack("!I", len(raw)) + raw)
            await writer.drain()

    async def _handle(self, writer, request, record):
        admission = [False]

        def admitted():
            admission[0] = True

        try:
            operation, payload = request["operation"], request["payload"]
            if operation == "hello":
                _fields(payload, set())
                if self._hello:
                    raise PrivateChannelError("invalid_request")
                self._hello = True
                result = {
                    "runtime_instance_id": self.actor.manager.runtime_instance_id,
                    "production_available": False,
                }
            else:
                if not self._hello:
                    raise PrivateChannelError("hello_required")
                result = await self._dispatch(operation, payload, admitted, record)
            await self._reply(writer, request, result=result)
        except (ValueError, RuntimeError) as error:
            code = getattr(error, "code", "operation_failed")
            allowed = {
                "invalid_request",
                "hello_required",
                "runtime_busy",
                "admission_closed",
                "runtime_replaced",
                "slot_replaced",
                "model_unavailable",
                "model_unsupported",
                "capability_unavailable",
                "model_not_found",
                "request_limit",
                "unsupported_contract",
                "native_runtime_unqualified",
                "native_runtime_unsupported",
                "invalid_load_plan",
                "load_admission_failed",
                "artifact_authority_unavailable",
                "artifact_custody_unavailable",
                "invalid_operation_ref",
                "unknown_or_expired_operation",
                "invalid_slot_ref",
                "caller_cancelled",
                "exchange_not_cancellable",
                "not_loading",
            }
            if type(code) is not str or code not in allowed:
                code = "operation_failed"
            await self._reply(
                writer,
                request,
                error={"code": code, "effect": "unknown" if admission[0] else "not_admitted"},
            )

    def _runtime(self, payload):
        if payload["runtime_instance_id"] != self.actor.manager.runtime_instance_id:
            raise PrivateChannelError("runtime_replaced")

    def _slot_ref(self, payload):
        from speech_binding import SpeechSlotRef

        self._runtime(payload)
        _fields(payload["slot"], {"slot_id", "load_generation"})
        return SpeechSlotRef(payload["runtime_instance_id"], **payload["slot"])

    def _handle_ref(self, payload):
        ref = self._slot_ref(payload)
        operation_id = payload["operation_id"]
        if type(operation_id) is not str:
            raise PrivateChannelError("invalid_operation_ref")
        handle = self._handles.get(operation_id)
        if handle is None or handle._native_ref.slot_ref != ref:
            raise PrivateChannelError("invalid_operation_ref")
        return handle

    async def _dispatch(self, operation, payload, admitted, record):
        if operation == "load":
            _fields(payload, {"runtime_instance_id", "model_id", "source_id"})
            self._runtime(payload)
            if record["cancel"]:
                raise PrivateChannelError("caller_cancelled")
            if self._gate is None or not hasattr(self._gate, "prepare_from_parent"):
                raise PrivateChannelError("native_runtime_unqualified")
            # This is an injected trusted internal dependency; the shipping
            # factory never implements it. JSON cannot install a qualifier.
            plan = self._gate.prepare_from_parent(payload, self.actor.manager)
            started = self.actor.start_load(plan, admission=admitted)
            record["load_ref"] = started.slot_ref
            status = await self.actor.wait_slot(started.slot_ref)
            if status.state == "ready":
                if self._bridge is not None:
                    self._bridge.close_admission()
                self._bridge = OwnedModelOperations(
                    self.actor,
                    model=plan.model_id,
                    profile=None,
                    slot_ref=status.slot_ref,
                    adapter=self._adapter,
                )
            return _load_status(status)
        if operation == "use":
            _fields(payload, {"runtime_instance_id", "slot", "request"})
            ref = self._slot_ref(payload)
            if self._bridge is None or ref != self._bridge._slot_ref:
                raise PrivateChannelError("slot_replaced")
            raw = json.dumps(payload["request"], ensure_ascii=False).encode("utf-8")
            handle = self._bridge.start(raw, admission=admitted)
            self._handles[handle._native_ref.operation_id] = handle
            # Native receipts are bounded. Local handles follow the same cap;
            # an evicted identity refuses status, never reconstructs/replays.
            while len(self._handles) > 64:
                del self._handles[next(iter(self._handles))]
            return _operation_status(self._bridge.status(handle))
        if operation == "status":
            _fields(payload, {"runtime_instance_id", "slot", "operation_id", "wait_for_settlement"})
            handle = self._handle_ref(payload)
            if type(payload["wait_for_settlement"]) is not bool:
                raise PrivateChannelError("invalid_request")
            admitted()
            if payload["wait_for_settlement"]:
                key = handle._native_ref.operation_id
                if key in self._waiters:
                    raise PrivateChannelError("runtime_busy")
                self._waiters.add(key)
                try:
                    status = await self._bridge.wait(handle)
                finally:
                    self._waiters.discard(key)
            else:
                status = self._bridge.status(handle)
            return _operation_status(status)
        if operation == "cancel":
            _fields(payload, {"runtime_instance_id", "slot", "operation_id"})
            handle = self._handle_ref(payload)
            admitted()
            return _operation_status(self._bridge.cancel(handle))
        if operation == "cancel_exchange":
            _fields(payload, {"target_exchange_id"})
            target = payload["target_exchange_id"]
            if type(target) is not int or not 0 < target <= (1 << 64) - 1:
                raise PrivateChannelError("invalid_request")
            original = self._records.get(target)
            if original is None or original["request"]["operation"] != "load":
                raise PrivateChannelError("exchange_not_cancellable")
            if original["load_ref"] is not None:
                self.actor.cancel_load(original["load_ref"])
            original["cancel"] = True
            return {"target_exchange_id": target, "cancellation_requested": True}
        if operation == "unload":
            _fields(payload, {"runtime_instance_id", "slot"})
            ref = self._slot_ref(payload)
            return _load_status(await self.actor.unload(ref, admission=admitted))
        if operation == "close":
            _fields(payload, {"runtime_instance_id"})
            self._runtime(payload)
            statuses = self.actor.close_admission()
            return {
                "runtime_instance_id": self.actor.manager.runtime_instance_id,
                "admission_closed": True,
                "custody_complete": all(item.cleanup == "confirmed" for item in statuses),
            }
        raise PrivateChannelError("invalid_request")


def create_private_owned_channel(manager):
    """Shipping factory: retain closed production gate, without fixture imports."""
    from owned_audio import OwnedAudioActor

    return PrivateOwnedChannel(OwnedAudioActor(manager))
