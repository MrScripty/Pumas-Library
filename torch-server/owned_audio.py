"""Private load-to-unload custody, exercised with controlled native workers.

There is no enabled production gate here. Installed interpreter/recipe/code and
the complete native consumed read set remain unqualified. In-process plans are
trusted dependencies, never decoded paths, manifests or permission receipts.
Quarantine survives caller loss and orderly loop shutdown; only the external
owner's exact child-tree cessation can discharge that unresolved custody.
"""

import asyncio
from collections import OrderedDict
from dataclasses import dataclass, field
from threading import Event, Thread
from typing import Any

import torch

from model_manager import LoadedModel, SlotState
from speech_binding import SpeechBindingError, SpeechSlotRef


_CUSTODIANS = set()
MAX_SETTLED_RECEIPTS = 64


class OwnedAudioError(ValueError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


class OwnedLoadPlan:
    """One-shot, private native-gate plan retaining load-time custody.

    The private gate is responsible for previously qualified byte/recipe/process
    evidence. It must retain any partially loaded native objects in this plan's
    custody, including when load raises. No shipped gate grants that authority.
    Serialization/copying cannot turn this object into another custody owner.
    """

    __slots__ = ("_gate", "runtime_instance_id", "model_id", "device", "_custody", "_claimed")

    def __init__(self, *args, **kwargs):
        raise TypeError("Owned load plans come from the private native gate")

    @classmethod
    def _from_native_gate(cls, gate, *, runtime_instance_id, model_id, custody, device="cpu"):
        plan = object.__new__(cls)
        plan._gate = gate
        plan.runtime_instance_id = runtime_instance_id
        plan.model_id = model_id
        plan.device = device
        plan._custody = custody
        plan._claimed = False
        return plan

    def __repr__(self):
        return "<OwnedLoadPlan>"

    def __reduce_ex__(self, protocol):
        raise TypeError("Owned load plans are not serializable")

    def __copy__(self):
        raise TypeError("Owned load plans are not copyable")

    def __deepcopy__(self, memo):
        raise TypeError("Owned load plans are not copyable")


class UnavailableOwnedNativeGate:
    """Fail closed until installed recipe and consumed read-set qualification."""

    def admit(self, plan, runtime_instance_id):
        raise OwnedAudioError("native_runtime_unqualified")


@dataclass(frozen=True)
class OwnedAudioStatus:
    slot_ref: SpeechSlotRef
    state: str
    cleanup: str
    cancellation_requested: bool
    outstanding_borrows: int
    error_code: str | None
    # Controlled workers do not establish production availability.
    production_available: bool = False


@dataclass
class _Entry:
    plan: OwnedLoadPlan = field(repr=False)
    slot: Any = field(repr=False)
    ref: SpeechSlotRef
    device: Any = field(repr=False)
    observed: asyncio.Future = field(repr=False)
    cancel: Event = field(default_factory=Event, repr=False)
    state: str = "loading"
    cleanup: str = "pending"
    error_code: str | None = None
    borrows: int = 0
    native_loaded: Any = field(default=None, repr=False)
    native_started: bool = False
    lock: Any = field(default=None, repr=False)
    runner: Any = field(default=None, repr=False)
    launch: Any = field(default=None, repr=False)
    worker: Any = field(default=None, repr=False)
    guard: Any = field(default=None, repr=False)
    retained_error: Any = field(default=None, repr=False)


class _Borrow:
    def __init__(self, actor, entry):
        self._actor = actor
        self._entry = entry
        self._released = False

    def validate(self, ref):
        self._actor._check_loop()
        if self._released or ref != self._entry.ref:
            raise SpeechBindingError("artifact_custody_unavailable")
        self._actor._ready(self._entry)

    def release(self):
        self._actor._check_loop()
        if self._released:
            return
        self._entry.borrows -= 1
        self._released = True


def _device_completion(device):
    if device.type == "cuda":
        torch.cuda.synchronize(device)
    elif device.type != "cpu":
        raise OwnedAudioError("native_runtime_unsupported")


class OwnedAudioActor:
    """Loop-owned admission, native workers and slot-lifetime artifact authority.

    The injected internal gate's admit is synchronous and nonblocking. Its load,
    cleanup and release_custody methods run on retained native workers. cleanup
    must dispose partial native state retained by the plan; release_custody must
    return only after its retained artifact owner confirms release. Exceptions
    cannot serve as cleanup receipts. Gate callbacks must not reenter the actor.
    No inference worker is added: SpeechOperationOwner uses this actor's borrows.
    """

    def __init__(self, manager, *, native_gate=None, max_settled_receipts=MAX_SETTLED_RECEIPTS):
        if type(max_settled_receipts) is not int or not 1 <= max_settled_receipts <= 64:
            raise ValueError("Settled receipt count must be between 1 and 64")
        self._loop = asyncio.get_running_loop()
        self._manager = manager
        self._runtime_instance_id = manager.runtime_instance_id
        self._gate = UnavailableOwnedNativeGate() if native_gate is None else native_gate
        self._entries = {}
        self._settled = OrderedDict()
        self._max_settled_receipts = max_settled_receipts
        self._active = None
        self._closed = False
        manager._install_owned_audio_actor(self)

    @property
    def manager(self):
        return self._manager

    @property
    def authority(self):
        return self

    def _check_loop(self):
        if asyncio.get_running_loop() is not self._loop:
            raise RuntimeError("Owned audio must run on its owning event loop")
        if self.manager.runtime_instance_id != self._runtime_instance_id:
            raise OwnedAudioError("runtime_replaced")

    def start_load(self, plan):
        self._check_loop()
        if self._closed:
            raise OwnedAudioError("admission_closed")
        if self._active is not None or any(
            e.cleanup != "confirmed" for e in self._entries.values()
        ):
            raise OwnedAudioError("runtime_busy")
        if (
            type(plan) is not OwnedLoadPlan
            or plan._gate is not self._gate
            or plan.runtime_instance_id != self._runtime_instance_id
            or plan._claimed
            or type(plan.model_id) is not str
            or not 1 <= len(plan.model_id) <= 256
        ):
            raise OwnedAudioError("invalid_load_plan")
        try:
            self._gate.admit(plan, self._runtime_instance_id)
        except BaseException as error:
            code = (
                error.code
                if type(error) is OwnedAudioError
                and error.code
                in {
                    "native_runtime_unqualified",
                    "native_runtime_unsupported",
                    "artifact_custody_unavailable",
                }
                else "load_admission_failed"
            )
            raise OwnedAudioError(code) from None
        try:
            device = self.manager.device_manager.resolve_device(plan.device)
        except BaseException:
            raise OwnedAudioError("load_admission_failed") from None
        if device.type not in ("cpu", "cuda"):
            raise OwnedAudioError("native_runtime_unsupported")
        slot = self.manager._reserve_owned_audio_slot(self, plan.model_id, device)
        try:
            ref = self.manager.speech_slot_ref(slot.slot_id)
            entry = _Entry(plan, slot, ref, device, self._loop.create_future())
            self._entries[ref] = entry
            _CUSTODIANS.add(self)
        except BaseException:
            self._entries.pop(self.manager.speech_slot_ref(slot.slot_id), None)
            self.manager._retire_owned_audio_slot(self, slot)
            raise
        # All retained state exists before one-shot transfer or native startup.
        plan._claimed = True
        self._active = entry
        self._launch(entry, self._run_load)
        return self._snapshot(entry)

    async def load(self, plan):
        status = self.start_load(plan)
        return await self.wait_slot(status.slot_ref)

    async def wait_slot(self, ref):
        self._check_loop()
        return await self._observe(self._find(ref))

    def cancel_load(self, ref):
        self._check_loop()
        entry = self._find(ref)
        if entry.state != "loading" or entry.cleanup != "pending":
            raise OwnedAudioError("not_loading")
        entry.cancel.set()
        return self._snapshot(entry)

    async def unload(self, ref):
        self._check_loop()
        entry = self._find(ref)
        self._ready(entry, allow_closed=True)
        if self._active is not None or entry.borrows:
            raise OwnedAudioError("runtime_busy")
        lock = self.manager._get_device_lock(entry.slot.device)
        if lock.locked():
            raise OwnedAudioError("runtime_busy")
        # Close exact-slot borrow admission before any suspension or worker call.
        entry.state = "unloading"
        entry.slot.state = SlotState.UNLOADING
        entry.cleanup = "pending"
        entry.observed = self._loop.create_future()
        self._active = entry
        self._launch(entry, self._run_unload)
        return await self._observe(entry)

    async def _observe(self, entry):
        try:
            await asyncio.shield(entry.observed)
        except asyncio.CancelledError:
            entry.cancel.set()
            raise
        return self._snapshot(entry)

    def _launch(self, entry, run):
        try:
            entry.launch = run(entry)
            task = self._loop.create_task(entry.launch)
            entry.runner = task
            task.add_done_callback(lambda done: self._runner_done(entry, done))
        except BaseException as error:
            self._quarantine(entry, "owner_start_unconfirmed", error)

    def _runner_done(self, entry, task):
        if entry.runner is task:
            entry.runner = None
        if entry.state in ("ready", "retired", "failed"):
            return
        error = None if task.cancelled() else task.exception()
        self._quarantine(entry, entry.error_code or "owner_cleanup_unconfirmed", error)

    def _find(self, ref):
        if type(ref) is not SpeechSlotRef:
            raise OwnedAudioError("invalid_slot_ref")
        if any(
            type(value) is not str
            for value in (
                ref.runtime_instance_id,
                ref.slot_id,
                ref.load_generation,
            )
        ):
            raise OwnedAudioError("invalid_slot_ref")
        if ref.runtime_instance_id != self._runtime_instance_id:
            raise OwnedAudioError("runtime_replaced")
        try:
            return self._entries[ref]
        except KeyError:
            raise OwnedAudioError("slot_replaced") from None

    def _ready(self, entry, *, allow_closed=False):
        if self._closed and not allow_closed:
            raise SpeechBindingError("artifact_custody_unavailable")
        slot = self.manager.slots.get(entry.ref.slot_id)
        if (
            entry.state != "ready"
            or slot is not entry.slot
            or slot.load_generation != entry.ref.load_generation
            or slot.state != SlotState.READY
            or slot._loaded is not entry.native_loaded
        ):
            raise SpeechBindingError("artifact_custody_unavailable")

    def acquire(self, ref):
        self._check_loop()
        try:
            entry = self._find(ref)
        except OwnedAudioError as error:
            raise SpeechBindingError(
                "runtime_replaced"
                if error.code == "runtime_replaced"
                else "artifact_custody_unavailable"
            ) from None
        self._ready(entry)
        if entry.borrows:
            raise SpeechBindingError("artifact_custody_unavailable")
        borrow = _Borrow(self, entry)
        entry.borrows += 1
        return borrow

    def status(self, ref):
        self._check_loop()
        entry = self._find(ref)
        if entry.state == "ready":
            self._ready(entry, allow_closed=True)
        return self._snapshot(entry)

    def _snapshot(self, entry):
        return OwnedAudioStatus(
            entry.ref,
            entry.state,
            entry.cleanup,
            entry.cancel.is_set(),
            entry.borrows,
            entry.error_code,
        )

    def close_admission(self):
        self._check_loop()
        self._closed = True
        if self._active is not None:
            self._active.cancel.set()
        if self.manager._speech_owner is not None:
            self.manager._speech_owner.close_admission()
        return tuple(self._snapshot(entry) for entry in self._entries.values())

    async def _resist(self, entry, future):
        while True:
            try:
                return await asyncio.shield(future)
            except asyncio.CancelledError:
                entry.cancel.set()

    async def _native(self, entry, invoke):
        observed = self._loop.create_future()
        outcome = []

        def receive():
            # Notification can precede the worker's last instruction. Observe
            # actual thread cessation before discharging native/device custody.
            if entry.worker.is_alive():
                self._loop.call_later(0.001, receive)
            elif not observed.done():
                observed.set_result(outcome.pop())

        def work():
            try:
                invoke()
                outcome.append(None)
            except BaseException as error:
                outcome.append(error)
            try:
                self._loop.call_soon_threadsafe(receive)
            except RuntimeError:
                pass  # Force-closed loop proves no cleanup; custody stays held.

        try:
            entry.worker = Thread(target=work, name="pumas-owned-audio", daemon=False)
            entry.worker.start()
        except BaseException as error:
            self._quarantine(entry, "worker_start_unconfirmed", error)
            await self._hold_quarantine(entry)
        error = await self._resist(entry, observed)
        entry.worker = None
        return error

    async def _take_device(self, entry):
        lock = self.manager._get_device_lock(entry.slot.device)
        if lock.locked():
            return False
        # An awakened earlier waiter can still make this acquisition suspend.
        await lock.acquire()
        entry.lock = lock
        return True

    async def _run_load(self, entry):
        entry.runner = asyncio.current_task()
        try:
            acquired = await self._take_device(entry)
            if entry.state == "cleanup_unconfirmed":
                await self._hold_quarantine(entry)
            if not acquired or entry.cancel.is_set() or self._closed:
                entry.error_code = "cancelled" if acquired else "runtime_busy"
                await self._retire(entry, native_cleanup=False)
                return

            def load():
                entry.native_started = True
                entry.native_loaded = self._gate.load(entry.plan, entry.device)

            error = await self._native(entry, load)
            if entry.state == "cleanup_unconfirmed":
                await self._hold_quarantine(entry)
            loaded = entry.native_loaded
            if error is not None:
                entry.error_code = "load_failed"
                entry.retained_error = error
            elif (
                type(loaded) is not LoadedModel
                or loaded.model is None
                or loaded.model_type != "cohere-asr"
                or str(loaded.device) != str(entry.device)
            ):
                entry.error_code = "invalid_native_result"
            elif entry.cancel.is_set() or self._closed:
                entry.error_code = "cancelled"
            if entry.error_code is not None:
                await self._retire(entry, native_cleanup=True)
                return
            error = await self._native(entry, lambda: _device_completion(entry.device))
            if entry.state == "cleanup_unconfirmed":
                await self._hold_quarantine(entry)
            if error is not None:
                self._quarantine(entry, "device_cleanup_unconfirmed", error)
                return
            if entry.cancel.is_set() or self._closed:
                entry.error_code = "cancelled"
                await self._retire(entry, native_cleanup=True)
                return
            self.manager._publish_owned_audio_slot(self, entry.slot, loaded)
            entry.state = "ready"
            entry.cleanup = "retained"
            entry.lock.release()
            entry.lock = None
            self._active = None
            entry.observed.set_result(None)
        except asyncio.CancelledError:
            entry.cancel.set()
            if not entry.native_started and entry.state != "cleanup_unconfirmed":
                entry.error_code = "cancelled"
                await self._retire(entry, native_cleanup=False)
            else:
                self._quarantine(entry, "owner_cleanup_unconfirmed", None)
        except BaseException as error:
            self._quarantine(entry, "owner_cleanup_unconfirmed", error)

    async def _run_unload(self, entry):
        entry.runner = asyncio.current_task()
        try:
            if not await self._take_device(entry):
                self._quarantine(entry, "device_cleanup_unconfirmed", None)
                return
            if entry.state == "cleanup_unconfirmed":
                await self._hold_quarantine(entry)
            await self._retire(entry, native_cleanup=True)
        except BaseException as error:
            self._quarantine(entry, "owner_cleanup_unconfirmed", error)

    async def _retire(self, entry, *, native_cleanup):
        if native_cleanup:

            def cleanup():
                _device_completion(entry.device)
                self._gate.cleanup(entry.plan, entry.native_loaded, entry.device)
                if type(entry.native_loaded) is LoadedModel:
                    entry.native_loaded.model = None
                    entry.native_loaded.tokenizer = None
                _device_completion(entry.device)

            error = await self._native(entry, cleanup)
            if error is not None:
                self._quarantine(entry, "native_cleanup_unconfirmed", error)
                return
        if entry.state == "cleanup_unconfirmed":
            await self._hold_quarantine(entry)
        # Retire only the original slot, before releasing its artifact owner.
        # A replacement or registry failure leaves custody retained/quarantined.
        self.manager._retire_owned_audio_slot(self, entry.slot)
        error = await self._native(entry, lambda: self._gate.release_custody(entry.plan))
        if error is not None:
            self._quarantine(entry, "artifact_cleanup_unconfirmed", error)
            return
        if entry.state == "cleanup_unconfirmed":
            await self._hold_quarantine(entry)
        entry.native_loaded = None
        entry.retained_error = None
        entry.plan = None
        entry.launch = None
        entry.cleanup = "confirmed"
        entry.state = "failed" if entry.error_code is not None else "retired"
        self._settled[entry.ref] = entry
        while len(self._settled) > self._max_settled_receipts:
            stale_ref, _ = self._settled.popitem(last=False)
            del self._entries[stale_ref]
        if entry.lock is not None:
            entry.lock.release()
            entry.lock = None
        self._active = None
        if not entry.observed.done():
            entry.observed.set_result(None)
        if all(item.cleanup == "confirmed" for item in self._entries.values()):
            _CUSTODIANS.discard(self)

    def _quarantine(self, entry, code, error):
        entry.state = "cleanup_unconfirmed"
        entry.cleanup = "unconfirmed"
        entry.error_code = code
        entry.retained_error = (entry.retained_error, error)
        entry.slot.state = SlotState.ERROR
        if not entry.observed.done():
            entry.observed.set_result(None)
        if entry.guard is None:
            entry.guard = asyncio.Task(self._hold_quarantine(entry), loop=self._loop)
            entry.guard.add_done_callback(lambda done: self._guard_done(entry, done))

    def _guard_done(self, entry, task):
        entry.guard = None
        if not task.cancelled():
            task.exception()
        self._quarantine(entry, entry.error_code, None)

    async def _hold_quarantine(self, entry):
        while True:
            try:
                await self._loop.create_future()
            except asyncio.CancelledError:
                entry.cancel.set()
