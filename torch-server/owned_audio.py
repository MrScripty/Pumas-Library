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


# Source-owned policy catalog. No installed interpreter/model read containment
# or native lifecycle policy is qualified, so the shipping catalog is empty.
# Tests temporarily install a fixed controlled policy; wire/env/metadata cannot.
_INSTALLED_AUDIO_POLICIES = ()


class _InstalledAudioSourceProof:
    """Opaque, single-use original source capability; never wire qualification."""

    __slots__ = ("_policy", "_source", "_identity", "_released", "_gate", "_lineage")

    def __init__(self, *args, **kwargs):
        raise TypeError("Installed source proofs come from a source-owned policy")

    @classmethod
    def _from_source_policy(cls, policy, source, *, manager, model_id, source_id):
        if not any(policy is candidate for candidate in _INSTALLED_AUDIO_POLICIES):
            raise OwnedAudioError("native_runtime_unqualified")
        runtime = manager.runtime_instance_id
        if any(
            type(value) is not str or not 1 <= len(value) <= 256
            for value in (runtime, model_id, source_id)
        ):
            raise OwnedAudioError("invalid_load_plan")
        # Blocking source inspection happens before gate/actor admission, not
        # inside its synchronous claim. This is a policy prerequisite, no claim
        # that package paths/digests or an import probe establish qualification.
        policy.validate_source(source)
        proof = object.__new__(cls)
        proof._policy, proof._source = policy, source
        proof._identity = (runtime, model_id, source_id, manager)
        proof._released, proof._gate, proof._lineage = False, None, None
        # The source owner must atomically retain one original proof. It may not
        # mint another proof/identity for an already retained/consumed source.
        policy.bind_retention(source, proof)
        return proof

    @property
    def runtime_instance_id(self):
        return self._identity[0]

    @property
    def model_id(self):
        return self._identity[1]

    @property
    def source_id(self):
        return self._identity[2]

    @property
    def manager(self):
        return self._identity[3]

    def __repr__(self):
        return "<InstalledAudioSourceProof>"

    def __reduce_ex__(self, protocol):
        raise TypeError("Installed source proofs are not serializable")

    def __copy__(self):
        raise TypeError("Installed source proofs are not copyable")

    def __deepcopy__(self, memo):
        raise TypeError("Installed source proofs are not copyable")


class _InstalledAudioCustody:
    def __init__(self, proof):
        from loaders.cohere_asr_loader import RetainedSpeechAcquisition

        self.proof = proof
        self.acquisition = RetainedSpeechAcquisition()
        self.loaded = None
        self.cleaned = False
        self.disposal = None


class _InstalledNativeDisposal:
    """One retained reference handoff; only the fixed policy can observe disposal."""

    def __init__(self, acquisition, loaded):
        reader = acquisition.objects.get("model_read_source")
        if (
            type(loaded) is not LoadedModel
            or loaded.model is None
            or loaded.tokenizer is None
            or reader is None
            or acquisition.unknown_allocations
            or acquisition.in_flight is not None
        ):
            raise OwnedAudioError("native_cleanup_unconfirmed")
        self.acquisition = acquisition
        self._reader = reader
        # Capture every wrapper alias before clearing the actor/slot wrapper.
        self._loaded_aliases = (loaded.model, loaded.tokenizer)
        self._references_released = False
        self._finished = False
        loaded.model = None
        loaded.tokenizer = None

    def release_native_references(self):
        acquisition = self.acquisition
        if (
            self._references_released
            or self._finished
            or acquisition.unknown_allocations
            or acquisition.in_flight is not None
            or acquisition.objects.get("model_read_source") is not self._reader
            or self._reader._closed
        ):
            raise OwnedAudioError("native_cleanup_unconfirmed")
        # Includes constructor tuples, eval aliases, weights and intermediates.
        # The original reader stays alive throughout native destruction.
        acquisition.objects.clear()
        acquisition.objects["model_read_source"] = self._reader
        self._loaded_aliases = ()
        self._references_released = True

    def finish_after_observation(self):
        if (
            not self._references_released
            or self._finished
            or self.acquisition.objects != {"model_read_source": self._reader}
            or self._reader._closed
        ):
            raise OwnedAudioError("native_cleanup_unconfirmed")
        self.acquisition.clear_after_cleanup()
        self._finished = True


class _InstalledOwnedNativeGate:
    """Conditional source plumbing only; the shipping policy catalog is empty.

    A future source-owned policy must validate complete original runtime/model
    capabilities with blocking validate_source, atomically bind one source proof
    with nonblocking bind_retention, observe actual native disposal before
    dispose_native returns and close actual model capabilities in release_source.
    Paths/manifests/import/version probes and a child boolean supply none of
    these authorities. Controlled tests qualify no installed runtime.
    """

    def __init__(self, *args, **kwargs):
        raise TypeError("Installed gates come from the original source owner")

    @classmethod
    def _from_source_owner(cls, source):
        if len(_INSTALLED_AUDIO_POLICIES) != 1:
            raise OwnedAudioError("native_runtime_unqualified")
        policy = _INSTALLED_AUDIO_POLICIES[0]
        proof = policy.retain_source(source)
        if (
            type(proof) is not _InstalledAudioSourceProof
            or proof._policy is not policy
            or proof._source is not source
            or proof._gate is not None
        ):
            raise OwnedAudioError("native_runtime_unqualified")
        gate = object.__new__(cls)
        gate._proof, gate._plan, gate._actor = proof, None, None
        gate._validate_memory()
        policy.validate_source(source)
        proof._gate = gate
        return gate

    def _validate_memory(self):
        proof = self._proof
        if not any(proof._policy is candidate for candidate in _INSTALLED_AUDIO_POLICIES):
            raise OwnedAudioError("native_runtime_unqualified")
        if proof._released:
            raise OwnedAudioError("artifact_custody_unavailable")
        if proof._gate is not None and proof._gate is not self:
            raise OwnedAudioError("invalid_load_plan")

    def bind_actor(self, manager, actor):
        """Original manager and actor identities only; no filesystem callback."""
        self._validate_memory()
        if (
            manager is not self._proof.manager
            or manager.runtime_instance_id != self._proof.runtime_instance_id
            or self._actor is not None
        ):
            raise OwnedAudioError("invalid_load_plan")
        self._actor = actor

    def prepare_from_parent(self, payload, manager):
        self._validate_memory()
        proof = self._proof
        if (
            type(payload) is not dict
            or payload.keys() != {"runtime_instance_id", "model_id", "source_id"}
            or any(type(value) is not str for value in payload.values())
            or payload["runtime_instance_id"] != proof.runtime_instance_id
            or manager is not proof.manager
            or manager.runtime_instance_id != proof.runtime_instance_id
            or payload["model_id"] != proof.model_id
            or payload["source_id"] != proof.source_id
            or (self._plan is not None and self._plan._claimed)
        ):
            raise OwnedAudioError("invalid_load_plan")
        # Wire preparation stays nonblocking. Source construction inspected the
        # retained capability; the native worker verifies its current bytes and
        # identity again before any native API/import/constructor effect.
        if self._plan is None:
            self._plan = OwnedLoadPlan._from_native_gate(
                self,
                runtime_instance_id=proof.runtime_instance_id,
                model_id=proof.model_id,
                custody=_InstalledAudioCustody(proof),
                device="cpu",
            )
        return self._plan

    def _plan_lineage(self, plan, *, claimed=False):
        self._validate_memory()
        proof = self._proof
        if (
            type(plan) is not OwnedLoadPlan
            or plan is not self._plan
            or plan._gate is not self
            or self._actor is None
            or self._actor.manager is not proof.manager
            or self._actor.manager.runtime_instance_id != proof.runtime_instance_id
            or proof.manager._owned_audio_actor is not self._actor
            or type(plan._custody) is not _InstalledAudioCustody
            or plan._custody.proof is not proof
            or plan.runtime_instance_id != proof.runtime_instance_id
            or plan.model_id != proof.model_id
            or (claimed and (plan._claimed is not True or proof._lineage is not plan))
        ):
            raise OwnedAudioError("invalid_load_plan")
        if plan.device != "cpu":
            raise OwnedAudioError("native_runtime_unsupported")

    def admit(self, plan, runtime_instance_id):
        # This actor/event-loop boundary performs retained in-memory checks only.
        self._plan_lineage(plan)
        if runtime_instance_id != self._proof.runtime_instance_id:
            raise OwnedAudioError("invalid_load_plan")

    def admit_device(self, plan, device):
        self._plan_lineage(plan)
        if device.type != "cpu" or str(device) != "cpu":
            raise OwnedAudioError("native_runtime_unsupported")
        if self._proof._lineage is None:
            if plan._claimed:
                raise OwnedAudioError("invalid_load_plan")
            self._proof._lineage = plan
        elif self._proof._lineage is not plan:
            raise OwnedAudioError("invalid_load_plan")

    def load(self, plan, device):
        from loaders.cohere_asr_loader import load_cohere_asr_retained
        from loaders.owned_cohere_source import HeldCohereReadSource

        self._plan_lineage(plan, claimed=True)
        self.admit_device(plan, device)
        custody = plan._custody
        if custody.loaded is not None or custody.cleaned or custody.acquisition.objects:
            raise OwnedAudioError("invalid_load_plan")
        # Original source and selected file capabilities are never submitted.
        self._proof._policy.validate_source(self._proof._source)
        source = self._proof._policy.model_source(self._proof._source)
        if type(source) is not HeldCohereReadSource:
            raise OwnedAudioError("artifact_custody_unavailable")
        custody.acquisition.retain("model_read_source", source)
        if source.source_owner is not self._proof._source:
            raise OwnedAudioError("artifact_custody_unavailable")
        self._proof._policy.validate_source(self._proof._source)
        model, processor, kind = load_cohere_asr_retained(source, device, custody.acquisition)
        custody.loaded = LoadedModel(model, processor, device, kind)
        return custody.loaded

    def cleanup(self, plan, loaded, device):
        self._plan_lineage(plan, claimed=True)
        self.admit_device(plan, device)
        custody = plan._custody
        if loaded is not custody.loaded or custody.cleaned:
            raise OwnedAudioError("invalid_load_plan")
        acquisition = custody.acquisition
        if acquisition.unknown_allocations or acquisition.in_flight is not None:
            # Dropping known Python objects cannot prove constructor-failure
            # native cessation. Keep every stage and original source in custody.
            raise OwnedAudioError("native_cleanup_unconfirmed")
        if loaded is None and not (acquisition.objects.keys() - {"model_read_source"}):
            # Refusal before the first native acquisition has nothing native to
            # dispose. A retained load traceback with native stages is different.
            acquisition.clear_after_cleanup()
            custody.cleaned = True
            return
        if custody.disposal is not None:
            raise OwnedAudioError("native_cleanup_unconfirmed")
        # Failed loads can retain native aliases in traceback frames even when
        # every constructor returned. Do not trim errors or claim their disposal.
        custody.disposal = _InstalledNativeDisposal(acquisition, loaded)
        self._proof._policy.dispose_native(custody.disposal, device)
        custody.disposal.finish_after_observation()
        custody.cleaned = True

    def release_custody(self, plan):
        self._plan_lineage(plan, claimed=True)
        custody = plan._custody
        acquisition = custody.acquisition
        if (
            acquisition.objects
            or (custody.disposal is not None and not custody.disposal._finished)
            or acquisition.unknown_allocations
            or acquisition.in_flight is not None
            or (
                custody.loaded is not None
                and (
                    not custody.cleaned
                    or custody.loaded.model is not None
                    or custody.loaded.tokenizer is not None
                )
            )
        ):
            raise OwnedAudioError("artifact_custody_unavailable")
        # Actual source/model descriptor closure belongs to its original owner.
        # Runtime roles remain held by the process parent through exact drain.
        self._proof._policy.release_source(self._proof._source)
        self._proof._released = True
        custody.loaded = None


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
        if hasattr(self._gate, "bind_actor"):
            self._gate.bind_actor(manager, self)
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

    def start_load(self, plan, *, admission=None):
        """Notify the private caller at claim, before launch or status can fail."""
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
        if hasattr(self._gate, "admit_device"):
            self._gate.admit_device(plan, device)
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
        if admission is not None:
            admission()
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

    async def unload(self, ref, *, admission=None):
        """Notify after clean refusals, at the exact-slot unload claim."""
        self._check_loop()
        entry = self._find(ref)
        self._ready(entry, allow_closed=True)
        if self._active is not None or entry.borrows:
            raise OwnedAudioError("runtime_busy")
        lock = self.manager._get_device_lock(entry.slot.device)
        if lock.locked():
            raise OwnedAudioError("runtime_busy")
        observed = self._loop.create_future()
        # Close exact-slot borrow admission before any suspension or worker call.
        entry.state = "unloading"
        entry.slot.state = SlotState.UNLOADING
        entry.cleanup = "pending"
        entry.observed = observed
        self._active = entry
        if admission is not None:
            admission()
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
