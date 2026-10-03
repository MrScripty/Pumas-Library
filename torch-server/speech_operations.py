"""Private, loop-owned finite ASR operations. No routes or capability advertisement.

Exact slot generations are private identities, not model/recipe attestation.
The default artifact authority refuses admission. Milestone B must implement
load-to-unload artifact custody and coordinate this owner with Pumas's existing
managed process owner. In particular, close/drain is NOT event-loop teardown:
quarantine deliberately survives task cancellation and prevents orderly asyncio
shutdown until the external process owner terminates the affected runtime. Never
force-close the loop or finalize its async generators while custody is incomplete.
"""

import asyncio
import base64
import binascii
from collections import OrderedDict
from dataclasses import dataclass, field
import hashlib
import json
import math
from threading import Event, Thread
import time
from typing import Any, Callable
from uuid import UUID, uuid4

from speech_binding import (
    BINDING_ERROR_CODES,
    ArtifactUseReleaseUnconfirmed,
    SpeechBindingError,
    SpeechSlotRef,
)

from loaders.cohere_asr_loader import (
    LANGUAGES,
    MAX_AUDIO_SAMPLES,
    MAX_TEXT_BYTES,
    SAMPLE_RATE,
    SpeechCancelled,
    SpeechCleanupUnconfirmed,
    SpeechRuntimeUnsupported,
    transcribe,
)

MAX_ENVELOPE_BYTES = 1_572_864
MAX_RECEIPTS = 64
RECEIPT_TTL_SECONDS = 600
# An HTTP client, or even its application-level owner, cannot drop live custody.
# Quarantined owners stay here until process exit. There is no release API.
_CUSTODIANS: set["SpeechOperationOwner"] = set()


class SpeechOperationError(ValueError):
    """A bounded, typed private-boundary refusal; contains no submitted data."""

    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


@dataclass(frozen=True)
class OperationRef:
    runtime_instance_id: str
    operation_id: str
    slot_ref: SpeechSlotRef


@dataclass(frozen=True)
class Diagnostic:
    code: str
    exception_type: str
    message: str


@dataclass(frozen=True)
class OperationStatus:
    operation_ref: OperationRef
    request_id: str
    state: str
    cancellation_requested: bool
    cleanup: str
    text: str | None
    operation_diagnostic: Diagnostic | None
    startup_diagnostic: Diagnostic | None
    owner_startup_diagnostic: Diagnostic | None
    cleanup_diagnostic: Diagnostic | None
    receipt_ttl_seconds: float
    max_settled_receipts: int
    # Monotonic provider time, never an expiry/replay guarantee to other clocks.
    expires_at: float | None


@dataclass(frozen=True)
class DrainReport:
    admission_closed: bool
    custody_complete: bool
    incomplete: tuple[OperationStatus, ...]


@dataclass
class _Outcome:
    text: str | None = None
    diagnostic: Diagnostic | None = None
    cleanup_diagnostic: Diagnostic | None = None
    quarantine: SpeechCleanupUnconfirmed | None = None


@dataclass
class _Entry:
    ref: OperationRef
    request_id: str
    digest: bytes
    audio: bytes | None = field(repr=False)
    language: str
    observed: asyncio.Future = field(repr=False)
    cancel: Event = field(default_factory=Event, repr=False)
    state: str = "running"
    cleanup: str = "pending"
    text: str | None = None
    diagnostic: Diagnostic | None = None
    startup_diagnostic: Diagnostic | None = None
    owner_startup_diagnostic: Diagnostic | None = None
    cleanup_diagnostic: Diagnostic | None = None
    settled_at: float | None = None
    runner: asyncio.Task | None = field(default=None, repr=False)
    worker: Thread | None = field(default=None, repr=False)
    lease: Any = field(default=None, repr=False)
    binding: Any = field(default=None, repr=False)
    launch: Any = field(default=None, repr=False)
    guard: asyncio.Task | None = field(default=None, repr=False)
    quarantine: Any = field(default=None, repr=False)


_DIAGNOSTIC_MESSAGES = {
    "cancelled": "Speech inference was cancelled.",
    "inference_failed": "Speech inference failed.",
    "runtime_unsupported": "The speech runtime is unsupported.",
    "model_unavailable": "The speech model is unavailable.",
    "slot_replaced": "The admitted speech slot is no longer current.",
    "runtime_replaced": "The admitted speech runtime is no longer current.",
    "invalid_slot_ref": "The speech slot reference is invalid.",
    "artifact_authority_unavailable": "Artifact-use authority is unavailable.",
    "artifact_custody_unavailable": "Attested artifact custody is unavailable.",
    "artifact_cleanup_unconfirmed": "Artifact-use borrow cleanup is unconfirmed.",
    "model_unsupported": "The selected model does not support speech inference.",
    "runtime_busy": "The speech device is busy.",
    "lease_refused": "The speech device lease was refused.",
    "binding_failed": "Speech artifact binding failed before native startup.",
    "owner_start_unconfirmed": "Operation runner startup is unconfirmed; runtime termination is required.",
    "owner_start_failed": "Operation runner startup reported an exception after confirmed cleanup.",
    "worker_start_unconfirmed": "Native worker startup is unconfirmed; runtime termination is required.",
    "device_cleanup_unconfirmed": "Speech device cleanup is unconfirmed.",
    "lease_cleanup_unconfirmed": "Speech lease cleanup is unconfirmed.",
    "owner_cleanup_unconfirmed": "Speech operation owner cleanup is unconfirmed.",
}
_SAFE_EXCEPTION_TYPES = (
    SpeechCleanupUnconfirmed,
    SpeechCancelled,
    SpeechRuntimeUnsupported,
    asyncio.CancelledError,
    KeyboardInterrupt,
    SystemExit,
    MemoryError,
    UnicodeError,
    ValueError,
    KeyError,
    RuntimeError,
    OSError,
    Exception,
    BaseException,
)


def _diagnostic(error: BaseException, code: str) -> Diagnostic:
    # Backend exception text/args/custom type names may contain PCM or transcripts.
    # Never invoke str/repr on the exception or copy its attributes into receipts.
    # Keep only an allowlisted category and code-owned, fixed bounded message.
    category = next(kind.__name__ for kind in _SAFE_EXCEPTION_TYPES if isinstance(error, kind))
    return Diagnostic(code, category, _DIAGNOSTIC_MESSAGES[code])


def _binding_code(error):
    return (
        error.code
        if type(error.code) is str and error.code in BINDING_ERROR_CODES
        else "artifact_custody_unavailable"
    )


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise SpeechOperationError("duplicate_field")
        result[key] = value
    return result


def _fields(value, expected):
    if type(value) is not dict or value.keys() != expected:
        raise SpeechOperationError("invalid_fields")


def _decode(body: bytes, runtime_id: str):
    # This must be the first boundary. Callers must enforce the same cap while
    # reading the transport stream; do not construct an unlimited body first.
    if type(body) is not bytes or len(body) > MAX_ENVELOPE_BYTES:
        raise SpeechOperationError("invalid_envelope")
    try:
        request = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object)
    except (UnicodeError, ValueError, RecursionError) as error:
        if isinstance(error, SpeechOperationError):
            raise
        raise SpeechOperationError("invalid_json") from None
    _fields(request, {"runtime_instance_id", "slot", "request_id", "language", "audio"})
    if request["runtime_instance_id"] != runtime_id:
        raise SpeechOperationError("runtime_replaced")
    slot = request["slot"]
    _fields(slot, {"slot_id", "load_generation"})
    try:
        if (
            type(slot["slot_id"]) is not str
            or not 1 <= len(slot["slot_id"]) <= 128
            or not slot["slot_id"].isascii()
            or type(slot["load_generation"]) is not str
            or str(UUID(slot["load_generation"])) != slot["load_generation"]
        ):
            raise ValueError
    except (ValueError, AttributeError):
        raise SpeechOperationError("invalid_slot_ref") from None
    slot_ref = SpeechSlotRef(runtime_id, slot["slot_id"], slot["load_generation"])
    request_id = request["request_id"]
    try:
        if type(request_id) is not str or str(UUID(request_id)) != request_id:
            raise ValueError
    except (ValueError, AttributeError):
        raise SpeechOperationError("invalid_request_id") from None
    language = request["language"]
    if type(language) is not str or language not in LANGUAGES:
        raise SpeechOperationError("invalid_language")
    audio = request["audio"]
    _fields(audio, {"encoding", "sample_rate_hz", "channels", "sample_count", "data_base64"})
    if (
        audio["encoding"] != "pcm_s16le"
        or type(audio["sample_rate_hz"]) is not int
        or audio["sample_rate_hz"] != SAMPLE_RATE
        or type(audio["channels"]) is not int
        or audio["channels"] != 1
        or type(audio["sample_count"]) is not int
        or not 1 <= audio["sample_count"] <= MAX_AUDIO_SAMPLES
    ):
        raise SpeechOperationError("invalid_audio_format")
    encoded = audio["data_base64"]
    byte_count = 2 * audio["sample_count"]
    if type(encoded) is not str or len(encoded) != 4 * ((byte_count + 2) // 3):
        raise SpeechOperationError("invalid_audio_length")
    try:
        pcm = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error):
        raise SpeechOperationError("invalid_base64") from None
    if len(pcm) != byte_count:
        raise SpeechOperationError("invalid_audio_length")
    # Refuse alternative pad bits and noncanonical encodings as well as garbage.
    if base64.b64encode(pcm).decode("ascii") != encoded:
        raise SpeechOperationError("invalid_base64")
    digest = hashlib.sha256()
    digest.update(
        json.dumps(
            [
                runtime_id,
                slot_ref.slot_id,
                slot_ref.load_generation,
                request_id,
                language,
                "pcm_s16le",
                SAMPLE_RATE,
                1,
                audio["sample_count"],
            ],
            separators=(",", ":"),
        ).encode("ascii")
    )
    digest.update(pcm)
    return slot_ref, request_id, language, pcm, digest.digest()


def _invoke(adapter: Callable, loaded: Any, audio: bytes, language: str, cancel: Event):
    """The thread publishes only a value snapshot, never a settled exception."""
    try:
        if cancel.is_set():
            raise SpeechCancelled("Speech inference cancelled before invocation")
        text = adapter(loaded.model, loaded.tokenizer, audio, language, cancel)
        if type(text) is not str or len(text) > MAX_TEXT_BYTES:
            raise ValueError("ASR runtime returned an invalid or oversized transcript")
        try:
            valid = len(text.encode("utf-8")) <= MAX_TEXT_BYTES
        except UnicodeError:
            valid = False
        if not valid:
            raise ValueError("ASR runtime returned an invalid or oversized transcript")
        return _Outcome(text=text)
    except SpeechCleanupUnconfirmed as error:
        return _Outcome(
            diagnostic=_diagnostic(error.original_error, "inference_failed")
            if error.original_error is not None
            else None,
            cleanup_diagnostic=_diagnostic(error.cleanup_error, "device_cleanup_unconfirmed"),
            quarantine=error,
        )
    except BaseException as error:
        if isinstance(error, SpeechCancelled):
            code = "cancelled"
        elif isinstance(error, SpeechRuntimeUnsupported):
            code = "runtime_unsupported"
        else:
            code = "inference_failed"
        return _Outcome(diagnostic=_diagnostic(error, code))


class SpeechOperationOwner:
    """One active operation, no queue; public methods belong to one event loop.

    start() has no await point: validation, deduplication and reservation are one
    atomic loop turn. Thread callers must marshal through the owning loop. The
    independently retained task and native worker are never returned to callers.
    Synthetic injection may tighten retention bounds, never enlarge defaults.
    """

    def __init__(
        self,
        manager,
        *,
        adapter=transcribe,
        clock=time.monotonic,
        max_receipts: int = MAX_RECEIPTS,
        receipt_ttl: float = RECEIPT_TTL_SECONDS,
    ):
        if type(max_receipts) is not int or not 1 <= max_receipts <= MAX_RECEIPTS:
            raise ValueError("Receipt count must be between 1 and 64")
        if type(receipt_ttl) not in (int, float) or not 0 < receipt_ttl <= RECEIPT_TTL_SECONDS:
            raise ValueError("Receipt lifetime must be between 0 and 600 seconds")
        self._loop = asyncio.get_running_loop()
        self._runtime_instance_id = manager.runtime_instance_id
        self._manager = manager
        self._adapter = adapter
        self._clock = clock
        self._max_receipts = max_receipts
        self._ttl = receipt_ttl
        self._entries: dict[str, _Entry] = {}
        self._requests: dict[str, _Entry] = {}
        self._settled: OrderedDict[str, _Entry] = OrderedDict()
        self._active: _Entry | None = None
        self._closed = False
        self._expiry: asyncio.TimerHandle | None = None
        manager._claim_speech_owner(self)

    @property
    def runtime_instance_id(self) -> str:
        return self._runtime_instance_id

    def _check_loop(self):
        if asyncio.get_running_loop() is not self._loop:
            raise RuntimeError("Speech operations must run on their owning event loop")

    def start(self, body: bytes) -> OperationStatus:
        self._check_loop()
        slot_ref, request_id, language, pcm, digest = _decode(body, self.runtime_instance_id)
        self._prune()
        existing = self._requests.get(request_id)
        if existing is not None:
            if existing.digest != digest:
                raise SpeechOperationError("request_conflict")
            return self._snapshot(existing)
        if self._closed:
            raise SpeechOperationError("admission_closed")
        if self._active is not None:
            raise SpeechOperationError("runtime_busy")
        try:
            # Finish allocating identity, observation and exact-slot state before
            # any authority can transfer a borrow to this operation.
            ref = OperationRef(self.runtime_instance_id, str(uuid4()), slot_ref)
            entry = _Entry(ref, request_id, digest, pcm, language, self._loop.create_future())
            entry.binding = self._manager.prepare_speech(slot_ref)
        except SpeechBindingError as error:
            raise SpeechOperationError(_binding_code(error)) from None
        except BaseException:
            raise SpeechOperationError("admission_failed") from None
        try:
            self._entries[ref.operation_id] = entry
            self._requests[request_id] = entry
            self._active = entry
            _CUSTODIANS.add(self)
        except BaseException:
            self._forget_admission(entry)
            raise SpeechOperationError("admission_failed") from None
        try:
            self._manager.bind_speech(entry.binding)
        except BaseException as error:
            if entry.binding.artifact_use is None:
                # acquire's contract transfers nothing on refusal. No native
                # launch has even been attempted, so this is a plain rejection.
                self._forget_admission(entry)
                code = (
                    _binding_code(error)
                    if isinstance(error, SpeechBindingError)
                    else "admission_failed"
                )
                raise SpeechOperationError(code) from None
            # A transfer completed before a later binding failure. Registration
            # already owns it; release only because native non-start is known.
            outcome = _Outcome(diagnostic=_diagnostic(error, "binding_failed"))
            try:
                entry.binding.release()
            except BaseException as cleanup:
                entry.diagnostic = outcome.diagnostic
                entry.cleanup_diagnostic = _diagnostic(cleanup, "artifact_cleanup_unconfirmed")
                entry.quarantine = (error, cleanup)
                self._quarantine_now(entry)
            else:
                self._settle(entry, outcome)
            return self._snapshot(entry)
        try:
            entry.launch = self._run(entry)
            runner = self._loop.create_task(entry.launch)
            self._retain_runner(entry, runner)
        except BaseException as error:
            if entry.settled_at is not None:
                # An eager runner can genuinely finish before its factory raises.
                # Its exact cleanup receipt remains authoritative, but do not
                # erase the separate, bounded factory-startup diagnostic.
                entry.owner_startup_diagnostic = _diagnostic(error, "owner_start_failed")
            else:
                # Factory failure cannot prove non-start, even if it returns no
                # task. Retain the original coroutine and any eager native work.
                entry.owner_startup_diagnostic = _diagnostic(error, "owner_start_unconfirmed")
                entry.quarantine = (entry.quarantine, error)
                self._quarantine_now(entry)
        return self._snapshot(entry)

    def _forget_admission(self, entry):
        self._entries.pop(entry.ref.operation_id, None)
        self._requests.pop(entry.request_id, None)
        if self._active is entry:
            self._active = None
        _CUSTODIANS.discard(self)
        entry.audio = None
        entry.binding = None

    def _retain_runner(self, entry, task):
        if entry.runner is not task:
            task.add_done_callback(lambda done: self._runner_done(entry, done))
            entry.runner = task

    def _quarantine_now(self, entry):
        self._mark_quarantine(entry)
        if entry.guard is None:
            # Emergency custody must not depend on the task factory that just
            # failed. This guard never invokes native work or resolves custody.
            entry.guard = asyncio.Task(self._hold_quarantine(entry), loop=self._loop)
            entry.guard.add_done_callback(lambda task: self._guard_done(entry, task))

    def _guard_done(self, entry, task):
        entry.guard = None
        if not task.cancelled():
            task.exception()  # Observe failure without exposing backend text.
        self._quarantine_now(entry)

    def status(self, ref: OperationRef) -> OperationStatus:
        return self._snapshot(self._find(ref))

    def cancel(self, ref: OperationRef) -> OperationStatus:
        entry = self._find(ref)
        if entry.settled_at is None:
            entry.cancel.set()
            if entry.state == "running":
                entry.state = "cancellation_requested"
        return self._snapshot(entry)

    async def wait(self, ref: OperationRef) -> OperationStatus:
        """Wait for a settled receipt OR quarantine; caller cancellation is local."""
        entry = self._find(ref)
        await asyncio.shield(entry.observed)
        # Holding an entry while waiting must not resurrect an expired receipt.
        return self.status(ref)

    def close_admission(self) -> DrainReport:
        self._check_loop()
        self._closed = True
        if self._active is not None:
            self.cancel(self._active.ref)
        return self._drain_report()

    async def drain(self, timeout: float = 0) -> DrainReport:
        """Bounded observation, not process/worker termination or a lease release."""
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or timeout < 0:
            raise ValueError("Drain observation timeout must be finite and nonnegative")
        self.close_admission()
        if self._active is not None and timeout:
            # asyncio.wait never cancels its inputs, on timeout OR caller cancel.
            await asyncio.wait({self._active.observed}, timeout=timeout)
        return self._drain_report()

    def _drain_report(self):
        incomplete = () if self._active is None else (self._snapshot(self._active),)
        return DrainReport(self._closed, not incomplete, incomplete)

    def _find(self, ref):
        self._check_loop()
        self._prune()
        if type(ref) is not OperationRef:
            raise SpeechOperationError("invalid_operation_ref")
        if ref.runtime_instance_id != self.runtime_instance_id:
            raise SpeechOperationError("runtime_replaced")
        try:
            entry = self._entries[ref.operation_id]
        except (KeyError, TypeError):
            raise SpeechOperationError("unknown_or_expired_operation") from None
        if ref != entry.ref:
            raise SpeechOperationError("invalid_operation_ref")
        return entry

    def _snapshot(self, entry):
        return OperationStatus(
            entry.ref,
            entry.request_id,
            entry.state,
            entry.cancel.is_set(),
            entry.cleanup,
            entry.text,
            entry.diagnostic,
            entry.startup_diagnostic,
            entry.owner_startup_diagnostic,
            entry.cleanup_diagnostic,
            self._ttl,
            self._max_receipts,
            None if entry.settled_at is None else entry.settled_at + self._ttl,
        )

    def _prune(self):
        now = self._clock()
        while self._settled:
            key, entry = next(iter(self._settled.items()))
            if len(self._settled) <= self._max_receipts and now - entry.settled_at < self._ttl:
                break
            del self._settled[key]
            del self._entries[key]
            del self._requests[entry.request_id]
        if self._expiry is not None:
            self._expiry.cancel()
            self._expiry = None
        if self._settled:
            first = next(iter(self._settled.values()))
            self._expiry = self._loop.call_later(
                max(0, first.settled_at + self._ttl - now), self._prune
            )

    async def _resist_cancellation(self, future, entry):
        while True:
            try:
                return await asyncio.shield(future)
            except asyncio.CancelledError:
                # Native execution continues. No cancelled asyncio future is used
                # as evidence of thread cessation.
                entry.cancel.set()
                if entry.state == "running":
                    entry.state = "cancellation_requested"

    async def _run(self, entry):
        # An eager factory can start this coroutine and then raise without ever
        # returning its task. Retain the actual runner before native acquisition.
        self._retain_runner(entry, asyncio.current_task())
        if entry.owner_startup_diagnostic is not None:
            await self._hold_quarantine(entry)
        if entry.cancel.is_set():
            await self._finish_without_worker(entry, _Outcome())
            return
        try:
            entry.lease = self._manager.speech_lease(entry.binding)
            loaded = await entry.lease.__aenter__()
        except BaseException as error:
            # An unlocked asyncio.Lock may still have an awakened earlier waiter;
            # acquisition can suspend. A factory-failure latch recorded during
            # that suspension must survive cancellation/refusal and cleanup.
            if isinstance(error, SpeechBindingError):
                code = _binding_code(error)
            elif isinstance(error, asyncio.CancelledError):
                code = "cancelled"
            elif isinstance(error, KeyError):
                code = "model_unavailable"
            elif isinstance(error, ValueError):
                code = "model_unsupported"
            elif isinstance(error, RuntimeError):
                code = "runtime_busy"
            else:
                code = "lease_refused"
            outcome = _Outcome(diagnostic=_diagnostic(error, code))
            if entry.owner_startup_diagnostic is not None:
                entry.diagnostic = outcome.diagnostic
                await self._hold_quarantine(entry)
            entry.lease = None
            await self._finish_without_worker(entry, outcome)
            return
        # Acquisition may have resumed after factory startup was quarantined.
        # Keep the acquired lease, and never cross into native worker startup.
        if entry.owner_startup_diagnostic is not None:
            await self._hold_quarantine(entry)
        notification = self._loop.create_future()
        result = []

        def receive():
            # The callback can race the thread's final instructions. Only actual
            # thread exit, not scheduling this callback, confirms worker cessation.
            if entry.worker.is_alive():
                self._loop.call_later(0.001, receive)
            elif not notification.done():
                outcome = result.pop()
                if (
                    entry.startup_diagnostic is not None
                    or entry.owner_startup_diagnostic is not None
                ):
                    # A startup exception already quarantined this operation.
                    # Preserve any later inference/device-cleanup diagnostics and
                    # conversion buffer, but never expose late successful text or
                    # turn an outcome notification into a release API.
                    entry.diagnostic = outcome.diagnostic
                    entry.cleanup_diagnostic = outcome.cleanup_diagnostic
                    if outcome.quarantine is not None:
                        entry.quarantine = (entry.quarantine, outcome.quarantine)
                    notification.set_result(None)
                else:
                    notification.set_result(outcome)

        def work():
            result.append(_invoke(self._adapter, loaded, entry.audio, entry.language, entry.cancel))
            try:
                self._loop.call_soon_threadsafe(receive)
            except RuntimeError:
                # A force-closed loop is NOT a cleanup receipt; retain custody.
                pass

        entry.worker = Thread(target=work, name="pumas-speech", daemon=False)
        try:
            entry.worker.start()
        except BaseException as error:
            # Thread.start may raise after native creation but before its start
            # acknowledgement is delivered. Neither is_alive()==False nor a
            # missing acknowledgement proves non-start. Fail closed without ever
            # exiting this lease; only the external process owner can resolve it.
            entry.startup_diagnostic = _diagnostic(error, "worker_start_unconfirmed")
            entry.quarantine = error
            await self._hold_quarantine(entry)
        else:
            outcome = await self._resist_cancellation(notification, entry)
        if entry.owner_startup_diagnostic is not None:
            if outcome is not None:
                entry.diagnostic = outcome.diagnostic
                entry.cleanup_diagnostic = outcome.cleanup_diagnostic
                if outcome.quarantine is not None:
                    entry.quarantine = (entry.quarantine, outcome.quarantine)
            await self._hold_quarantine(entry)
        if outcome.quarantine is not None:
            entry.quarantine = outcome.quarantine
            entry.diagnostic = outcome.diagnostic
            entry.cleanup_diagnostic = outcome.cleanup_diagnostic
            await self._hold_quarantine(entry)
        # The real lease releases its artifact borrow and then the shared device
        # only on this confirmed-cleanup path. Exceptional exit retains custody.
        try:
            await entry.lease.__aexit__(None, None, None)
        except BaseException as error:
            entry.diagnostic = outcome.diagnostic
            code = (
                "artifact_cleanup_unconfirmed"
                if isinstance(error, ArtifactUseReleaseUnconfirmed)
                else "lease_cleanup_unconfirmed"
            )
            entry.cleanup_diagnostic = _diagnostic(error, code)
            entry.quarantine = error
            await self._hold_quarantine(entry)
        entry.lease = None
        self._settle(entry, outcome)

    async def _release_artifact(self, entry, outcome):
        if entry.owner_startup_diagnostic is not None:
            entry.diagnostic = outcome.diagnostic
            await self._hold_quarantine(entry)
        try:
            entry.binding.release()
        except BaseException as error:
            entry.diagnostic = outcome.diagnostic
            entry.cleanup_diagnostic = _diagnostic(error, "artifact_cleanup_unconfirmed")
            entry.quarantine = error
            await self._hold_quarantine(entry)

    async def _finish_without_worker(self, entry, outcome):
        # Local preworker cancellation/refusal cannot discharge an already
        # latched factory-startup uncertainty, even after a suspended acquisition.
        if entry.owner_startup_diagnostic is not None:
            entry.diagnostic = outcome.diagnostic
            await self._hold_quarantine(entry)
        # Without that uncertainty, these paths establish exact native non-start.
        await self._release_artifact(entry, outcome)
        self._settle(entry, outcome)

    def _mark_quarantine(self, entry):
        entry.state = "cleanup_unconfirmed"
        entry.cleanup = "unconfirmed"
        entry.text = None
        if not entry.observed.done():
            entry.observed.set_result(None)

    async def _hold_quarantine(self, entry):
        self._mark_quarantine(entry)
        # This live task prevents normal asyncio shutdown from finalizing the
        # speech_lease async generator prematurely. The process owner must stop
        # this runtime; cancellation, timeout and drain cannot resolve quarantine.
        while True:
            try:
                await self._loop.create_future()
            except asyncio.CancelledError:
                entry.cancel.set()

    def _settle(self, entry, outcome):
        entry.diagnostic = outcome.diagnostic
        if outcome.diagnostic is not None and outcome.diagnostic.code != "cancelled":
            entry.state = "failed"
        elif entry.cancel.is_set() or outcome.diagnostic is not None:
            entry.state = "cancelled"
        else:
            entry.state = "completed"
            entry.text = outcome.text
        entry.cleanup = "confirmed"
        entry.audio = None
        entry.worker = None
        entry.binding = None
        entry.launch = None
        entry.settled_at = self._clock()
        self._active = None
        self._settled[entry.ref.operation_id] = entry
        self._prune()
        _CUSTODIANS.discard(self)
        if not entry.observed.done():
            entry.observed.set_result(None)

    def _runner_done(self, entry, task):
        entry.runner = None
        if entry.settled_at is not None:
            return
        if entry.state == "cleanup_unconfirmed":
            if not task.cancelled():
                task.exception()
            self._quarantine_now(entry)
            return
        if task.cancelled() and entry.lease is None and entry.worker is None:
            # Cancellation before the owner coroutine's first instruction cannot
            # have started native work or acquired a lease.
            entry.cancel.set()
            entry.runner = asyncio.Task(
                self._finish_without_worker(entry, _Outcome()), loop=self._loop
            )
            entry.runner.add_done_callback(lambda task: self._runner_done(entry, task))
            return
        error = task.exception() if not task.cancelled() else asyncio.CancelledError()
        if error is not None:
            entry.cleanup_diagnostic = _diagnostic(error, "owner_cleanup_unconfirmed")
        self._quarantine_now(entry)
