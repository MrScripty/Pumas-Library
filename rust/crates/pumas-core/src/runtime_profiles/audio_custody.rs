//! Private exact-child retention for selected audio bytes.
//!
//! This owner is not recipe/read-set qualification. Production admission remains
//! unavailable; an installed runtime and its complete loader read set must first
//! be independently qualified. No caller paths, manifests, or identity JSON can
//! create qualification. The fixture seam below exists only in unit tests.
//!
//! A child holds one composite cleanup lease. ManagedChild parks that lease on
//! uncertain drainage, and drops it only after complete process-tree drainage.
//! Per-operation borrows consult retained memory only. Dropping an admitted load,
//! unload, or unsettled native borrow retains uncertainty, never releases bytes.

#![allow(dead_code)] // The private native channel is a subsequent integration seam.

use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::models::RuntimeProfileId;
use crate::platform::managed_child::ManagedChild;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioCustodyError {
    UnqualifiedRuntime,
    Unavailable,
    StaleIdentity,
    Busy,
    Uncertain,
    Poisoned,
    IdentityExhausted,
}

type Result<T> = std::result::Result<T, AudioCustodyError>;

/// Comparison values from the owning channel, not independently an authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioSlotIdentity {
    pub(crate) runtime_instance: String,
    pub(crate) slot_id: String,
    pub(crate) load_generation: String,
}

/// Existing native OperationRef comparison, received on the original owning
/// start exchange. It cannot construct a borrow or attest selected bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioOperationIdentity {
    pub(crate) slot: AudioSlotIdentity,
    pub(crate) operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reserved,
    Admitted,
    Ready,
    Unloading,
    Uncertain,
}

struct Entry {
    // Erasure occurs only after the concrete prepared-byte owner is accepted.
    // There is no production API accepting arbitrary erased custody.
    _bytes: Arc<dyn Send + Sync>,
    phase: Phase,
    slot: Option<AudioSlotIdentity>,
    borrows: usize,
}

#[derive(Default)]
struct State {
    pid: Option<u32>,
    child_attached: bool,
    child_drained: bool,
    runtime_instance: Option<String>,
    next_load: u64,
    entries: HashMap<u64, Entry>,
}

pub(crate) struct AudioCustodyRegistry {
    profile_id: RuntimeProfileId,
    generation: u64,
    admission_closed: AtomicBool,
    state: Mutex<State>,
}

impl fmt::Debug for AudioCustodyRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AudioCustodyRegistry")
            .field("profile_id", &self.profile_id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl AudioCustodyRegistry {
    pub(crate) fn new(profile_id: RuntimeProfileId, generation: u64) -> Arc<Self> {
        Arc::new(Self {
            profile_id,
            generation,
            admission_closed: AtomicBool::new(false),
            state: Mutex::new(State::default()),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state.lock().map_err(|_| AudioCustodyError::Poisoned)
    }

    pub(crate) fn close_admission(&self) {
        self.admission_closed.store(true, Ordering::Release);
    }

    fn admission_open(&self) -> Result<()> {
        if self.admission_closed.load(Ordering::Acquire) {
            Err(AudioCustodyError::Unavailable)
        } else {
            Ok(())
        }
    }

    /// Called once, immediately after spawn and before any ready publication.
    /// The cleanup guard is never handed to a request or retained separately.
    pub(crate) fn attach_to_child(self: &Arc<Self>, child: &mut ManagedChild) -> Result<()> {
        let mut state = self.lock()?;
        if state.child_attached || state.child_drained {
            return Err(AudioCustodyError::StaleIdentity);
        }
        state.pid = Some(child.id());
        state.child_attached = true;
        child.attach_cleanup_lease(Arc::new(ChildCleanupLease(self.clone())));
        Ok(())
    }

    fn check_session(
        &self,
        state: &State,
        profile: &RuntimeProfileId,
        generation: u64,
        pid: u32,
    ) -> Result<()> {
        if self.profile_id != *profile || self.generation != generation || state.pid != Some(pid) {
            return Err(AudioCustodyError::StaleIdentity);
        }
        if !state.child_attached || state.child_drained {
            return Err(AudioCustodyError::Unavailable);
        }
        Ok(())
    }

    /// Exact inherited-channel handshake. This binds lineage, not qualification.
    /// A second/different runtime instance cannot replace this child’s binding.
    pub(crate) fn bind_native_instance(
        &self,
        profile: &RuntimeProfileId,
        generation: u64,
        pid: u32,
        runtime_instance: &str,
    ) -> Result<()> {
        let mut state = self.lock()?;
        self.admission_open()?;
        self.check_session(&state, profile, generation, pid)?;
        if !canonical_uuid(runtime_instance) {
            return Err(AudioCustodyError::StaleIdentity);
        }
        match &state.runtime_instance {
            Some(current) if current != runtime_instance => Err(AudioCustodyError::StaleIdentity),
            Some(_) => Ok(()),
            None => {
                state.runtime_instance = Some(runtime_instance.to_owned());
                Ok(())
            }
        }
    }

    /// No real recipe/read-set qualifier exists yet. Neither a successful
    /// handshake nor caller-supplied identity values can open this gate.
    pub(crate) fn reserve_prepared(
        self: &Arc<Self>,
        _prepared: Arc<PreparedArtifactUse>,
    ) -> Result<AudioLoadAdmission> {
        Err(AudioCustodyError::UnqualifiedRuntime)
    }

    // Only the future trusted qualification path may call this after concrete
    // PreparedArtifactUse acquisition and pre-effect validation. Tests use a
    // synthetic destructor witness, without claiming execution qualification.
    fn retain(&self, state: &mut State, bytes: Arc<dyn Send + Sync>) -> Result<u64> {
        self.admission_open()?;
        if !state.child_attached || state.child_drained || state.runtime_instance.is_none() {
            return Err(AudioCustodyError::Unavailable);
        }
        // First bounded slice admits one load/slot per exact child. This also
        // preserves the existing root-wide exclusive prepared-byte grant.
        if !state.entries.is_empty() {
            return Err(AudioCustodyError::Busy);
        }
        state.next_load = state
            .next_load
            .checked_add(1)
            .ok_or(AudioCustodyError::IdentityExhausted)?;
        let token = state.next_load;
        state.entries.insert(
            token,
            Entry {
                _bytes: bytes,
                phase: Phase::Reserved,
                slot: None,
                borrows: 0,
            },
        );
        Ok(token)
    }

    fn mark_uncertain(&self, token: u64) {
        if let Ok(mut state) = self.lock() {
            if let Some(entry) = state.entries.get_mut(&token) {
                entry.phase = Phase::Uncertain;
            }
        }
    }

    fn child_drained(&self) {
        self.close_admission();
        let entries = match self.lock() {
            Ok(mut state) => {
                state.child_drained = true;
                std::mem::take(&mut state.entries)
            }
            // Poison is not evidence that retained bookkeeping is coherent.
            Err(_) => return,
        };
        // Scratch disposal can perform filesystem work; never do it under the
        // registry lock or on a synchronous per-operation borrow path.
        drop(entries);
    }

    #[cfg(test)]
    fn reserve_fixture(
        self: &Arc<Self>,
        bytes: Arc<dyn Send + Sync>,
    ) -> Result<AudioLoadAdmission> {
        let token = {
            let mut state = self.lock()?;
            self.retain(&mut state, bytes)?
        };
        Ok(AudioLoadAdmission {
            registry: self.clone(),
            token,
            armed: true,
        })
    }
}

impl Drop for AudioCustodyRegistry {
    fn drop(&mut self) {
        let state = match self.state.get_mut() {
            Ok(state) if state.child_drained => return,
            Ok(state) => state,
            Err(poison) => poison.into_inner(),
        };
        // Unexpected last-owner loss/poison never disposes unresolved bytes.
        // Normally the attached child lease prevents this path altogether.
        std::mem::forget(std::mem::take(&mut state.entries));
    }
}

struct ChildCleanupLease(Arc<AudioCustodyRegistry>);

impl Drop for ChildCleanupLease {
    fn drop(&mut self) {
        // ManagedChild retains this guard while parked or incompletely drained.
        self.0.child_drained();
    }
}

/// Owned by the admitted exchange worker, not the caller's response future.
pub(crate) struct AudioLoadAdmission {
    registry: Arc<AudioCustodyRegistry>,
    token: u64,
    armed: bool,
}

impl AudioLoadAdmission {
    /// Must precede the first wire effect. Even a partial write is now owned.
    pub(crate) fn mark_wire_admitted(&mut self) -> Result<()> {
        let mut state = self.registry.lock()?;
        self.registry.admission_open()?;
        if state.child_drained {
            return Err(AudioCustodyError::Unavailable);
        }
        let entry = state
            .entries
            .get_mut(&self.token)
            .ok_or(AudioCustodyError::Unavailable)?;
        if entry.phase != Phase::Reserved {
            return Err(AudioCustodyError::Uncertain);
        }
        entry.phase = Phase::Admitted;
        Ok(())
    }

    /// Called only for a fully validated original-channel load response.
    pub(crate) fn ready(mut self, slot: AudioSlotIdentity) -> Result<AudioLoadedSlot> {
        {
            let mut state = self.registry.lock()?;
            if state.child_drained
                || state.runtime_instance.as_deref() != Some(slot.runtime_instance.as_str())
                || !valid_slot(&slot)
            {
                return Err(AudioCustodyError::StaleIdentity);
            }
            let entry = state
                .entries
                .get_mut(&self.token)
                .ok_or(AudioCustodyError::Unavailable)?;
            if entry.phase != Phase::Admitted {
                return Err(AudioCustodyError::Uncertain);
            }
            entry.slot = Some(slot.clone());
            entry.phase = Phase::Ready;
        }
        self.armed = false;
        Ok(AudioLoadedSlot {
            registry: self.registry.clone(),
            token: self.token,
            slot,
        })
    }

    /// A failed original load may be reusable only when the owning channel
    /// confirms native/device cleanup for this exact admission. Public JSON
    /// claiming cleanup, a valid RPC error alone, or a closed transport cannot
    /// call this trusted seam. The load token is retained in this receipt.
    pub(crate) fn finish_native_cleaned(mut self, runtime_instance: &str) -> Result<()> {
        let entry = {
            let mut state = self.registry.lock()?;
            if state.child_drained || state.runtime_instance.as_deref() != Some(runtime_instance) {
                return Err(AudioCustodyError::StaleIdentity);
            }
            let entry = state
                .entries
                .get(&self.token)
                .ok_or(AudioCustodyError::Unavailable)?;
            if entry.phase != Phase::Admitted || entry.borrows != 0 {
                return Err(AudioCustodyError::Uncertain);
            }
            state.entries.remove(&self.token)
        };
        self.armed = false;
        drop(entry);
        Ok(())
    }
}

impl Drop for AudioLoadAdmission {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let entry = match self.registry.lock() {
            Ok(mut state) => match state.entries.get_mut(&self.token) {
                Some(entry) if entry.phase == Phase::Reserved => state.entries.remove(&self.token),
                Some(entry) => {
                    entry.phase = Phase::Uncertain;
                    None
                }
                None => None,
            },
            Err(_) => None,
        };
        drop(entry);
    }
}

/// A retained slot handle. Dropping the observation does not unload its slot.
pub(crate) struct AudioLoadedSlot {
    registry: Arc<AudioCustodyRegistry>,
    token: u64,
    slot: AudioSlotIdentity,
}

impl AudioLoadedSlot {
    pub(crate) fn identity(&self) -> &AudioSlotIdentity {
        &self.slot
    }

    pub(crate) fn borrow_operation(&self) -> Result<AudioOperationBorrow> {
        let mut state = self.registry.lock()?;
        self.registry.admission_open()?;
        let entry = checked_entry(&mut state, self.token, &self.slot, Phase::Ready)?;
        if entry.borrows != 0 {
            return Err(AudioCustodyError::Busy);
        }
        entry.borrows = 1;
        Ok(AudioOperationBorrow {
            registry: self.registry.clone(),
            token: self.token,
            slot: self.slot.clone(),
            operation_id: None,
            armed: true,
        })
    }

    pub(crate) fn begin_unload(&self) -> Result<AudioUnloadAdmission> {
        let mut state = self.registry.lock()?;
        let entry = checked_entry(&mut state, self.token, &self.slot, Phase::Ready)?;
        if entry.borrows != 0 {
            return Err(AudioCustodyError::Busy);
        }
        entry.phase = Phase::Unloading;
        Ok(AudioUnloadAdmission {
            registry: self.registry.clone(),
            token: self.token,
            slot: self.slot.clone(),
            armed: true,
        })
    }
}

/// Noncloneable, in-memory borrow. Caller cancellation cannot imply settlement.
pub(crate) struct AudioOperationBorrow {
    registry: Arc<AudioCustodyRegistry>,
    token: u64,
    slot: AudioSlotIdentity,
    operation_id: Option<String>,
    armed: bool,
}

impl AudioOperationBorrow {
    pub(crate) fn bind_native_operation(&mut self, receipt: &AudioOperationIdentity) -> Result<()> {
        self.validate()?;
        if receipt.slot != self.slot
            || !canonical_uuid(&receipt.operation_id)
            || self.operation_id.is_some()
        {
            self.registry.mark_uncertain(self.token);
            return Err(AudioCustodyError::StaleIdentity);
        }
        self.operation_id = Some(receipt.operation_id.clone());
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let mut state = self.registry.lock()?;
        let entry = checked_entry(&mut state, self.token, &self.slot, Phase::Ready)?;
        if entry.borrows != 1 {
            return Err(AudioCustodyError::Uncertain);
        }
        Ok(())
    }

    /// Only the owning channel's matching native/device-settled receipt may
    /// invoke this. A task/HTTP response closing is never such a receipt.
    pub(crate) fn finish_native_settled(mut self, receipt: &AudioOperationIdentity) -> Result<()> {
        if receipt.slot != self.slot
            || self.operation_id.as_deref() != Some(receipt.operation_id.as_str())
        {
            return Err(AudioCustodyError::StaleIdentity);
        }
        {
            let mut state = self.registry.lock()?;
            let entry = checked_entry(&mut state, self.token, &self.slot, Phase::Ready)?;
            if entry.borrows != 1 {
                return Err(AudioCustodyError::Uncertain);
            }
            entry.borrows = 0;
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for AudioOperationBorrow {
    fn drop(&mut self) {
        if self.armed {
            self.registry.mark_uncertain(self.token);
        }
    }
}

pub(crate) struct AudioUnloadAdmission {
    registry: Arc<AudioCustodyRegistry>,
    token: u64,
    slot: AudioSlotIdentity,
    armed: bool,
}

impl AudioUnloadAdmission {
    /// Exact native unload/device cessation, with no outstanding borrows.
    pub(crate) fn finish_native_unloaded(mut self, receipt: &AudioSlotIdentity) -> Result<()> {
        if receipt != &self.slot {
            return Err(AudioCustodyError::StaleIdentity);
        }
        let entry = {
            let mut state = self.registry.lock()?;
            let entry = checked_entry(&mut state, self.token, &self.slot, Phase::Unloading)?;
            if entry.borrows != 0 {
                return Err(AudioCustodyError::Busy);
            }
            state.entries.remove(&self.token)
        };
        self.armed = false;
        drop(entry);
        Ok(())
    }
}

impl Drop for AudioUnloadAdmission {
    fn drop(&mut self) {
        if self.armed {
            self.registry.mark_uncertain(self.token);
        }
    }
}

fn checked_entry<'a>(
    state: &'a mut State,
    token: u64,
    slot: &AudioSlotIdentity,
    phase: Phase,
) -> Result<&'a mut Entry> {
    if state.child_drained {
        return Err(AudioCustodyError::Unavailable);
    }
    let entry = state
        .entries
        .get_mut(&token)
        .ok_or(AudioCustodyError::Unavailable)?;
    if entry.slot.as_ref() != Some(slot) {
        return Err(AudioCustodyError::StaleIdentity);
    }
    if entry.phase != phase {
        return Err(AudioCustodyError::Uncertain);
    }
    Ok(entry)
}

fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value && !id.is_nil())
}

fn valid_slot(slot: &AudioSlotIdentity) -> bool {
    canonical_uuid(&slot.runtime_instance)
        && canonical_uuid(&slot.load_generation)
        && !slot.slot_id.is_empty()
        && slot.slot_id.len() <= 128
        && slot
            .slot_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    const INSTANCE: &str = "10000000-0000-0000-0000-000000000001";
    const LOAD: &str = "20000000-0000-0000-0000-000000000001";

    struct Bytes(Arc<AtomicUsize>);
    impl Drop for Bytes {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct Fixture {
        registry: Arc<AudioCustodyRegistry>,
        child: Option<ChildCleanupLease>,
        released: Arc<AtomicUsize>,
    }

    impl Fixture {
        fn new() -> Self {
            let registry = AudioCustodyRegistry::new(profile(), 7);
            {
                let mut state = registry.lock().unwrap();
                state.pid = Some(42);
                state.child_attached = true;
            }
            registry
                .bind_native_instance(&profile(), 7, 42, INSTANCE)
                .unwrap();
            Self {
                child: Some(ChildCleanupLease(registry.clone())),
                registry,
                released: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn load(&self) -> AudioLoadAdmission {
            self.registry
                .reserve_fixture(Arc::new(Bytes(self.released.clone())))
                .unwrap()
        }

        fn ready(&self) -> AudioLoadedSlot {
            let mut load = self.load();
            load.mark_wire_admitted().unwrap();
            load.ready(slot()).unwrap()
        }

        fn assert_releases(&self, expected: usize) {
            assert_eq!(self.released.load(Ordering::SeqCst), expected);
        }

        fn drain(&mut self) {
            drop(self.child.take());
        }
    }

    fn profile() -> RuntimeProfileId {
        RuntimeProfileId::parse("owned-audio-fixture").unwrap()
    }

    fn slot() -> AudioSlotIdentity {
        AudioSlotIdentity {
            runtime_instance: INSTANCE.into(),
            slot_id: "cafe1234".into(),
            load_generation: LOAD.into(),
        }
    }

    fn operation() -> AudioOperationIdentity {
        AudioOperationIdentity {
            slot: slot(),
            operation_id: "30000000-0000-0000-0000-000000000001".into(),
        }
    }

    fn bound_operation(loaded: &AudioLoadedSlot) -> AudioOperationBorrow {
        let mut borrowed = loaded.borrow_operation().unwrap();
        borrowed.bind_native_operation(&operation()).unwrap();
        borrowed
    }

    #[test]
    fn queued_caller_loss_releases_only_before_wire_admission() {
        let mut fixture = Fixture::new();
        drop(fixture.load());
        fixture.assert_releases(1);
        let mut admitted = fixture.load();
        admitted.mark_wire_admitted().unwrap();
        drop(admitted);
        fixture.assert_releases(1);
        assert!(matches!(
            fixture.registry.reserve_fixture(Arc::new(())),
            Err(AudioCustodyError::Busy)
        ));
        fixture.drain();
        fixture.assert_releases(2);
    }

    #[test]
    fn matching_native_settlement_and_unload_release_exact_slot() {
        let fixture = Fixture::new();
        let loaded = fixture.ready();
        let borrowed = bound_operation(&loaded);
        borrowed.validate().unwrap();
        assert!(matches!(
            loaded.borrow_operation(),
            Err(AudioCustodyError::Busy)
        ));
        assert!(matches!(
            loaded.begin_unload(),
            Err(AudioCustodyError::Busy)
        ));
        borrowed.finish_native_settled(&operation()).unwrap();
        fixture.assert_releases(0);
        loaded
            .begin_unload()
            .unwrap()
            .finish_native_unloaded(&slot())
            .unwrap();
        fixture.assert_releases(1);
        assert!(matches!(
            loaded.borrow_operation(),
            Err(AudioCustodyError::Unavailable)
        ));
        let next = fixture.ready();
        assert!(matches!(
            loaded.begin_unload(),
            Err(AudioCustodyError::Unavailable)
        ));
        next.begin_unload()
            .unwrap()
            .finish_native_unloaded(&slot())
            .unwrap();
        fixture.assert_releases(2);
    }

    #[test]
    fn caller_loss_after_native_borrow_retains_uncertainty() {
        let mut fixture = Fixture::new();
        let loaded = fixture.ready();
        drop(loaded.borrow_operation().unwrap());
        assert!(matches!(
            loaded.begin_unload(),
            Err(AudioCustodyError::Uncertain)
        ));
        assert!(matches!(
            loaded.borrow_operation(),
            Err(AudioCustodyError::Uncertain)
        ));
        fixture.assert_releases(0);
        fixture.drain();
        fixture.assert_releases(1);
    }

    #[test]
    fn wrong_ready_instance_generation_and_malformed_slot_keep_custody() {
        for wrong in [
            AudioSlotIdentity {
                runtime_instance: LOAD.into(),
                ..slot()
            },
            AudioSlotIdentity {
                load_generation: "unknown".into(),
                ..slot()
            },
            AudioSlotIdentity {
                slot_id: "../outside".into(),
                ..slot()
            },
        ] {
            let mut fixture = Fixture::new();
            let mut load = fixture.load();
            load.mark_wire_admitted().unwrap();
            assert!(matches!(
                load.ready(wrong),
                Err(AudioCustodyError::StaleIdentity)
            ));
            fixture.assert_releases(0);
            assert_eq!(
                fixture
                    .registry
                    .lock()
                    .unwrap()
                    .entries
                    .values()
                    .next()
                    .unwrap()
                    .phase,
                Phase::Uncertain
            );
            fixture.drain();
            fixture.assert_releases(1);
        }
    }

    #[test]
    fn stale_runtime_process_and_profile_cannot_replace_binding() {
        let fixture = Fixture::new();
        for (profile, generation, pid, runtime) in [
            (profile(), 8, 42, INSTANCE),
            (profile(), 7, 43, INSTANCE),
            (
                RuntimeProfileId::parse("other-profile").unwrap(),
                7,
                42,
                INSTANCE,
            ),
            (profile(), 7, 42, LOAD),
        ] {
            assert_eq!(
                fixture
                    .registry
                    .bind_native_instance(&profile, generation, pid, runtime),
                Err(AudioCustodyError::StaleIdentity)
            );
        }
        assert_eq!(
            fixture
                .registry
                .bind_native_instance(&profile(), 7, 42, INSTANCE),
            Ok(())
        );
    }

    #[test]
    fn wrong_native_settled_receipt_quarantines_borrow() {
        let mut fixture = Fixture::new();
        let loaded = fixture.ready();
        let wrong = AudioOperationIdentity {
            slot: AudioSlotIdentity {
                load_generation: INSTANCE.into(),
                ..slot()
            },
            ..operation()
        };
        assert_eq!(
            bound_operation(&loaded).finish_native_settled(&wrong),
            Err(AudioCustodyError::StaleIdentity)
        );
        assert!(matches!(
            loaded.begin_unload(),
            Err(AudioCustodyError::Uncertain)
        ));
        fixture.assert_releases(0);
        fixture.drain();
        fixture.assert_releases(1);
    }

    #[test]
    fn unknown_unload_and_wrong_unloaded_receipt_retain_bytes() {
        for wrong_receipt in [false, true] {
            let mut fixture = Fixture::new();
            let loaded = fixture.ready();
            let unload = loaded.begin_unload().unwrap();
            if wrong_receipt {
                let wrong = AudioSlotIdentity {
                    runtime_instance: LOAD.into(),
                    ..slot()
                };
                assert_eq!(
                    unload.finish_native_unloaded(&wrong),
                    Err(AudioCustodyError::StaleIdentity)
                );
            } else {
                drop(unload);
            }
            fixture.assert_releases(0);
            assert!(matches!(
                loaded.begin_unload(),
                Err(AudioCustodyError::Uncertain)
            ));
            fixture.drain();
            fixture.assert_releases(1);
        }
    }

    #[test]
    fn verified_clean_native_load_failure_reuses_child_without_releasing_unknown_failure() {
        let mut fixture = Fixture::new();
        let mut load = fixture.load();
        load.mark_wire_admitted().unwrap();
        load.finish_native_cleaned(INSTANCE).unwrap();
        fixture.assert_releases(1);
        let loaded = fixture.ready();
        loaded
            .begin_unload()
            .unwrap()
            .finish_native_unloaded(&slot())
            .unwrap();
        fixture.assert_releases(2);
        let mut wrong = fixture.load();
        wrong.mark_wire_admitted().unwrap();
        assert_eq!(
            wrong.finish_native_cleaned(LOAD),
            Err(AudioCustodyError::StaleIdentity)
        );
        fixture.assert_releases(2);
        assert!(matches!(
            fixture.registry.reserve_fixture(Arc::new(())),
            Err(AudioCustodyError::Busy)
        ));
        fixture.drain();
        fixture.assert_releases(3);
    }

    #[test]
    fn old_completed_operation_cannot_settle_new_same_slot_borrow() {
        let mut fixture = Fixture::new();
        let loaded = fixture.ready();
        bound_operation(&loaded)
            .finish_native_settled(&operation())
            .unwrap();
        let next_identity = AudioOperationIdentity {
            operation_id: "30000000-0000-0000-0000-000000000002".into(),
            ..operation()
        };
        let mut next = loaded.borrow_operation().unwrap();
        next.bind_native_operation(&next_identity).unwrap();
        assert_eq!(
            next.finish_native_settled(&operation()),
            Err(AudioCustodyError::StaleIdentity)
        );
        fixture.assert_releases(0);
        assert!(matches!(
            loaded.begin_unload(),
            Err(AudioCustodyError::Uncertain)
        ));
        fixture.drain();
        fixture.assert_releases(1);
    }

    #[test]
    fn missing_or_wrong_start_binding_never_authorizes_native_settlement() {
        for wrong_binding in [false, true] {
            let mut fixture = Fixture::new();
            let loaded = fixture.ready();
            let mut borrowed = loaded.borrow_operation().unwrap();
            if wrong_binding {
                let wrong = AudioOperationIdentity {
                    operation_id: "invalid".into(),
                    ..operation()
                };
                assert_eq!(
                    borrowed.bind_native_operation(&wrong),
                    Err(AudioCustodyError::StaleIdentity)
                );
            }
            assert_eq!(
                borrowed.finish_native_settled(&operation()),
                Err(AudioCustodyError::StaleIdentity)
            );
            fixture.assert_releases(0);
            fixture.drain();
            fixture.assert_releases(1);
        }
    }

    #[test]
    fn close_refuses_new_work_but_retains_existing_cleanup_authority() {
        let fixture = Fixture::new();
        let loaded = fixture.ready();
        let borrowed = bound_operation(&loaded);
        fixture.registry.close_admission();
        borrowed.validate().unwrap();
        borrowed.finish_native_settled(&operation()).unwrap();
        assert!(matches!(
            loaded.borrow_operation(),
            Err(AudioCustodyError::Unavailable)
        ));
        loaded
            .begin_unload()
            .unwrap()
            .finish_native_unloaded(&slot())
            .unwrap();
        fixture.assert_releases(1);
    }

    #[test]
    fn last_request_owner_loss_is_pinned_by_exact_child_cleanup_lease() {
        let mut fixture = Fixture::new();
        let mut admitted = fixture.load();
        admitted.mark_wire_admitted().unwrap();
        drop(admitted);
        let weak = Arc::downgrade(&fixture.registry);
        let child = fixture.child.take().unwrap();
        let released = fixture.released.clone();
        drop(fixture);
        assert!(weak.upgrade().is_some());
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(child);
        assert!(weak.upgrade().is_none());
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn poisoned_bookkeeping_never_releases_retained_bytes_even_after_child_drain() {
        let mut fixture = Fixture::new();
        let mut admitted = fixture.load();
        admitted.mark_wire_admitted().unwrap();
        let registry = fixture.registry.clone();
        assert!(std::thread::spawn(move || {
            let _held = registry.state.lock().unwrap();
            panic!("controlled registry poison");
        })
        .join()
        .is_err());
        drop(admitted);
        fixture.drain();
        fixture.assert_releases(0);
        let released = fixture.released.clone();
        drop(fixture);
        assert_eq!(released.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn concrete_prepared_bytes_retain_scratch_and_root_until_exact_native_unload() {
        use crate::index::ModelRecord;
        use crate::model_library::{DownloadDestinationRoot, DownloadPersistence, ModelLibrary};
        use crate::models::{AssetValidationState, ModelMetadata, StorageKind};

        let temp = tempfile::TempDir::new().unwrap();
        let library = ModelLibrary::new(temp.path().join("library"))
            .await
            .unwrap();
        let model_id = "audio/synthetic-owned";
        let artifact_id = "synthetic-selection";
        let package = library.library_root().join(model_id);
        std::fs::create_dir_all(&package).unwrap();
        let members = [
            "config.json",
            "model.safetensors",
            "preprocessor_config.json",
            "tokenizer.json",
            "tokenizer_config.json",
        ];
        for name in members {
            let bytes: &[u8] = match name {
                "config.json" => br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
                "model.safetensors" => b"synthetic bytes; no tensor loading",
                _ => b"{}",
            };
            std::fs::write(package.join(name), bytes).unwrap();
        }
        let metadata = serde_json::to_value(ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.into()),
            selected_artifact_id: Some(artifact_id.into()),
            selected_artifact_files: Some(members.iter().map(|member| (*member).into()).collect()),
            storage_kind: Some(StorageKind::LibraryOwned),
            validation_state: Some(AssetValidationState::Valid),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(
            package.join("metadata.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        library
            .index()
            .upsert(&ModelRecord {
                id: model_id.into(),
                path: package.display().to_string(),
                cleaned_name: "synthetic-owned".into(),
                official_name: "Synthetic selected-byte fixture".into(),
                model_type: "audio".into(),
                tags: vec![],
                hashes: HashMap::new(),
                metadata,
                updated_at: "fixture".into(),
            })
            .unwrap();
        let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
        library
            .install_mutation_authority(
                crate::api::RuntimeTasks::new(),
                root.clone(),
                Arc::new(DownloadPersistence::new(&temp.path().join("downloads"))),
            )
            .unwrap();

        let fixture = Fixture::new();
        let prepared = Arc::new(
            library
                .prepare_cohere_artifact_use(model_id, artifact_id)
                .unwrap(),
        );
        assert!(matches!(
            fixture.registry.reserve_prepared(prepared.clone()),
            Err(AudioCustodyError::UnqualifiedRuntime)
        ));
        prepared.validate_read_source().unwrap();
        let scratch = prepared.read_source_path().to_owned();
        let mut admitted = fixture.registry.reserve_fixture(prepared).unwrap();
        admitted.mark_wire_admitted().unwrap();
        let loaded = admitted.ready(slot()).unwrap();
        assert!(scratch.exists());
        assert!(matches!(
            root.try_acquire_execution_grant(),
            Err(crate::PumasError::DownloadRootBusy)
        ));
        let borrowed = bound_operation(&loaded);
        borrowed.finish_native_settled(&operation()).unwrap();
        assert!(scratch.exists());
        loaded
            .begin_unload()
            .unwrap()
            .finish_native_unloaded(loaded.identity())
            .unwrap();
        assert!(!scratch.exists());
        drop(root.try_acquire_execution_grant().unwrap());
    }

    #[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
    #[test]
    fn parked_managed_child_keeps_actual_composite_lease_until_tree_drain() {
        use crate::platform::managed_child::ManagedChildCustodySlot;
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let custody = ManagedChildCustodySlot::new();
        let registry = AudioCustodyRegistry::new(profile(), 9);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "read held"]).stdin(Stdio::piped());
        let mut child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        registry.attach_to_child(&mut child).unwrap();
        assert_eq!(
            registry.attach_to_child(&mut child),
            Err(AudioCustodyError::StaleIdentity)
        );
        registry
            .bind_native_instance(&profile(), 9, child.id(), INSTANCE)
            .unwrap();
        let released = Arc::new(AtomicUsize::new(0));
        let mut admitted = registry
            .reserve_fixture(Arc::new(Bytes(released.clone())))
            .unwrap();
        admitted.mark_wire_admitted().unwrap();
        drop(admitted);
        drop(child);
        assert!(custody.has_parked_child());
        assert_eq!(released.load(Ordering::SeqCst), 0);
        assert!(!registry.lock().unwrap().child_drained);
        custody.drain(Duration::from_secs(5)).unwrap();
        assert!(registry.lock().unwrap().child_drained);
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
}
