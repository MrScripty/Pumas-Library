//! Source-agnostic observations of concurrent network transfers.
//!
//! Producers report cumulative samples for each transfer. A transfer ID identifies one
//! stream of samples; an operation ID groups streams belonging to one user operation.
//! The registry never combines different transfers or measurement bases into one byte
//! total, since doing so could count the same traffic twice.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OperationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransferId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Download,
    Upload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    Active,
    Idle,
    Completed,
    Failed,
    Cancelled,
}

/// What the byte counter measures. Payload and wire bytes must not be added together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementBasis {
    Payload,
    Wire,
    Unknown,
}

/// How much of the transfer's traffic the producer can observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementCoverage {
    Complete,
    Partial,
    Unknown,
    Unsupported,
}

/// Safe display/copy metadata. URLs retain only their origin, omitting credentials,
/// path, query, and fragment. Labels are caller-provided and must contain no secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyableSource {
    url: Option<String>,
    label: Option<String>,
}

impl CopyableSource {
    pub fn new(url: Option<&str>, label: Option<&str>) -> Self {
        let url = url.and_then(|raw| {
            let parsed = Url::parse(raw).ok()?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
                return None;
            }
            Some(parsed.origin().ascii_serialization())
        });
        let label = label
            .map(|raw| {
                raw.chars()
                    .filter(|ch| !ch.is_control())
                    .take(120)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|label| !label.is_empty());
        Self { url, label }
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

/// One cumulative observation. `None` means the producer has no byte count.
/// A decrease in `cumulative_bytes` starts a new rate baseline for a retry/reset.
#[derive(Debug, Clone)]
pub struct ActivityUpdate {
    pub operation_id: OperationId,
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub source: Option<CopyableSource>,
    pub expected_bytes: Option<u64>,
    pub cumulative_bytes: Option<u64>,
    pub state: TransferState,
    pub basis: MeasurementBasis,
    pub coverage: MeasurementCoverage,
    pub sampled_at: Instant,
}

#[derive(Debug, Clone)]
pub struct TransferActivity {
    pub operation_id: OperationId,
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub source: Option<CopyableSource>,
    pub expected_bytes: Option<u64>,
    pub cumulative_bytes: Option<u64>,
    /// Bytes per second over the latest valid pair of samples. `None` means no
    /// reliable rate is available; an observed idle/final transfer has rate zero.
    pub rate_bytes_per_second: Option<f64>,
    pub state: TransferState,
    pub basis: MeasurementBasis,
    pub coverage: MeasurementCoverage,
    pub sampled_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityError {
    /// A transfer ID cannot be reused for a different operation or direction.
    TransferIdentityConflict,
    /// An older sample cannot overwrite a newer observation.
    OutOfOrderSample,
}

#[derive(Debug, Clone)]
struct Entry {
    activity: TransferActivity,
    rate_baseline: Option<(u64, Instant)>,
}

/// A thread-safe registry of independent transfer observations.
#[derive(Debug, Default)]
pub struct NetworkActivityRegistry {
    transfers: Mutex<HashMap<TransferId, Entry>>,
}

impl NetworkActivityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update one transfer and return its current snapshot.
    pub fn update(&self, update: ActivityUpdate) -> Result<TransferActivity, ActivityError> {
        let mut transfers = self
            .transfers
            .lock()
            .expect("network activity registry poisoned");
        if let Some(entry) = transfers.get_mut(&update.transfer_id) {
            if entry.activity.operation_id != update.operation_id
                || entry.activity.direction != update.direction
            {
                return Err(ActivityError::TransferIdentityConflict);
            }
            if update.sampled_at < entry.activity.sampled_at {
                return Err(ActivityError::OutOfOrderSample);
            }

            let measurable = update.basis != MeasurementBasis::Unknown
                && matches!(
                    update.coverage,
                    MeasurementCoverage::Complete | MeasurementCoverage::Partial
                );
            let same_measurement =
                entry.activity.basis == update.basis && entry.activity.coverage == update.coverage;
            let reset = update
                .cumulative_bytes
                .zip(entry.activity.cumulative_bytes)
                .is_some_and(|(current, previous)| current < previous);
            let rate = if update.state != TransferState::Active {
                Some(0.0)
            } else if measurable && same_measurement && !reset {
                match (entry.rate_baseline, update.cumulative_bytes) {
                    (Some((previous, at)), Some(current)) if current >= previous => update
                        .sampled_at
                        .checked_duration_since(at)
                        .filter(|elapsed| !elapsed.is_zero())
                        .map(|elapsed| (current - previous) as f64 / elapsed.as_secs_f64()),
                    _ => None,
                }
            } else {
                None
            };

            // A zero-time sample remains pending against the earlier baseline. A
            // counter reset or measurement change establishes a fresh baseline.
            if update.state != TransferState::Active
                || reset
                || !same_measurement
                || update.cumulative_bytes.is_none()
                || entry.rate_baseline.is_none()
                || update.sampled_at > entry.rate_baseline.unwrap().1
            {
                entry.rate_baseline = update
                    .cumulative_bytes
                    .map(|bytes| (bytes, update.sampled_at));
            }
            if let Some(source) = update.source {
                entry.activity.source = Some(source);
            }
            if let Some(expected) = update.expected_bytes {
                entry.activity.expected_bytes = Some(expected);
            }
            entry.activity.cumulative_bytes = update.cumulative_bytes;
            entry.activity.rate_bytes_per_second = rate;
            entry.activity.state = update.state;
            entry.activity.basis = update.basis;
            entry.activity.coverage = update.coverage;
            entry.activity.sampled_at = update.sampled_at;
            Ok(entry.activity.clone())
        } else {
            let activity = TransferActivity {
                operation_id: update.operation_id,
                transfer_id: update.transfer_id.clone(),
                direction: update.direction,
                source: update.source,
                expected_bytes: update.expected_bytes,
                cumulative_bytes: update.cumulative_bytes,
                rate_bytes_per_second: (update.state != TransferState::Active).then_some(0.0),
                state: update.state,
                basis: update.basis,
                coverage: update.coverage,
                sampled_at: update.sampled_at,
            };
            let rate_baseline = update
                .cumulative_bytes
                .map(|bytes| (bytes, update.sampled_at));
            transfers.insert(
                update.transfer_id,
                Entry {
                    activity: activity.clone(),
                    rate_baseline,
                },
            );
            Ok(activity)
        }
    }

    pub fn get(&self, transfer_id: &TransferId) -> Option<TransferActivity> {
        self.transfers
            .lock()
            .expect("network activity registry poisoned")
            .get(transfer_id)
            .map(|entry| entry.activity.clone())
    }

    pub fn for_operation(&self, operation_id: &OperationId) -> Vec<TransferActivity> {
        let mut activities = self
            .transfers
            .lock()
            .expect("network activity registry poisoned")
            .values()
            .filter(|entry| &entry.activity.operation_id == operation_id)
            .map(|entry| entry.activity.clone())
            .collect::<Vec<_>>();
        activities.sort_by(|a, b| a.transfer_id.0.cmp(&b.transfer_id.0));
        activities
    }

    pub fn snapshot(&self) -> Vec<TransferActivity> {
        let mut activities = self
            .transfers
            .lock()
            .expect("network activity registry poisoned")
            .values()
            .map(|entry| entry.activity.clone())
            .collect::<Vec<_>>();
        activities.sort_by(|a, b| a.transfer_id.0.cmp(&b.transfer_id.0));
        activities
    }

    pub fn remove(&self, transfer_id: &TransferId) -> Option<TransferActivity> {
        self.transfers
            .lock()
            .expect("network activity registry poisoned")
            .remove(transfer_id)
            .map(|entry| entry.activity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn sample(operation: &str, transfer: &str, bytes: Option<u64>, at: Instant) -> ActivityUpdate {
        ActivityUpdate {
            operation_id: OperationId(operation.into()),
            transfer_id: TransferId(transfer.into()),
            direction: TransferDirection::Download,
            source: None,
            expected_bytes: None,
            cumulative_bytes: bytes,
            state: TransferState::Active,
            basis: MeasurementBasis::Payload,
            coverage: MeasurementCoverage::Complete,
            sampled_at: at,
        }
    }

    #[test]
    fn concurrent_operations_are_independent() {
        let registry = Arc::new(NetworkActivityRegistry::new());
        let at = Instant::now();
        std::thread::scope(|scope| {
            for (operation, transfer, bytes) in
                [("a", "one", 10), ("b", "two", 30), ("a", "three", 50)]
            {
                let registry = Arc::clone(&registry);
                scope.spawn(move || {
                    registry
                        .update(sample(operation, transfer, Some(bytes), at))
                        .unwrap();
                });
            }
        });
        assert_eq!(registry.for_operation(&OperationId("a".into())).len(), 2);
        assert_eq!(registry.for_operation(&OperationId("b".into())).len(), 1);
        assert_eq!(registry.snapshot().len(), 3);
        assert_eq!(
            registry
                .update(sample("b", "one", Some(20), at))
                .unwrap_err(),
            ActivityError::TransferIdentityConflict
        );
    }

    #[test]
    fn rate_uses_cumulative_delta_and_elapsed_time() {
        let registry = NetworkActivityRegistry::new();
        let at = Instant::now();
        assert!(registry
            .update(sample("a", "one", Some(100), at))
            .unwrap()
            .rate_bytes_per_second
            .is_none());
        let activity = registry
            .update(sample("a", "one", Some(300), at + Duration::from_secs(2)))
            .unwrap();
        assert_eq!(activity.rate_bytes_per_second, Some(100.0));
        assert_eq!(activity.cumulative_bytes, Some(300));
        assert_eq!(
            registry
                .update(sample("a", "one", Some(300), at + Duration::from_secs(3)))
                .unwrap()
                .rate_bytes_per_second,
            Some(0.0)
        );
    }

    #[test]
    fn idle_and_final_states_clear_rate() {
        let registry = NetworkActivityRegistry::new();
        let at = Instant::now();
        registry.update(sample("a", "one", Some(10), at)).unwrap();
        let mut next = sample("a", "one", Some(20), at + Duration::from_secs(1));
        assert_eq!(
            registry.update(next.clone()).unwrap().rate_bytes_per_second,
            Some(10.0)
        );
        next.sampled_at += Duration::from_secs(1);
        next.state = TransferState::Idle;
        assert_eq!(
            registry.update(next.clone()).unwrap().rate_bytes_per_second,
            Some(0.0)
        );
        next.sampled_at += Duration::from_secs(1);
        next.state = TransferState::Completed;
        assert_eq!(
            registry.update(next).unwrap().rate_bytes_per_second,
            Some(0.0)
        );
    }

    #[test]
    fn counter_reset_starts_a_new_rate_baseline() {
        let registry = NetworkActivityRegistry::new();
        let at = Instant::now();
        registry.update(sample("a", "one", Some(100), at)).unwrap();
        let reset = registry
            .update(sample("a", "one", Some(10), at + Duration::from_secs(1)))
            .unwrap();
        assert_eq!(reset.rate_bytes_per_second, None);
        assert_eq!(
            registry
                .update(sample("a", "one", Some(30), at + Duration::from_secs(2)))
                .unwrap()
                .rate_bytes_per_second,
            Some(20.0)
        );
    }

    #[test]
    fn unknown_coverage_does_not_claim_a_rate() {
        let registry = NetworkActivityRegistry::new();
        let at = Instant::now();
        let mut first = sample("a", "one", None, at);
        first.basis = MeasurementBasis::Unknown;
        first.coverage = MeasurementCoverage::Unsupported;
        registry.update(first.clone()).unwrap();
        first.sampled_at += Duration::from_secs(1);
        first.cumulative_bytes = Some(100);
        let activity = registry.update(first).unwrap();
        assert_eq!(activity.coverage, MeasurementCoverage::Unsupported);
        assert_eq!(activity.rate_bytes_per_second, None);
    }

    #[test]
    fn copyable_source_omits_sensitive_url_components() {
        let source = CopyableSource::new(
            Some("https://user:password@example.com/private/token?key=secret#fragment"),
            Some("  source\nlabel  "),
        );
        assert_eq!(source.url(), Some("https://example.com"));
        assert_eq!(source.label(), Some("sourcelabel"));
    }
}
