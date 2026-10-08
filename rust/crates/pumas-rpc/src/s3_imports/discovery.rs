//! Discovery uses the existing bounded worker, never a workspace or consumer.
use super::*;
use pumas_library::acquisition::{S3PrefixError, S3PrefixLimits, S3Reader, S3ReaderError};

pub(crate) struct DiscoveryJob {
    pub(super) id: String,
    reader: S3Reader,
    prefix: String,
    deadline: Duration,
    cancelled: watch::Receiver<bool>,
}
pub(super) struct CurrentDiscovery {
    id: String,
    pub(super) cancel: watch::Sender<bool>,
    pub(super) result: Option<S3DiscoveryOutcome>,
}
impl S3Imports {
    pub(crate) fn start_discovery(
        &self,
        request: S3DiscoveryParams,
        credentials: Option<S3CredentialParams>,
    ) -> Result<S3DiscoveryOutcome> {
        let prepare = || -> std::result::Result<S3Reader, PublicError> {
            request.validate()?;
            let config = S3ReaderConfig {
                endpoint: request.endpoint.clone(),
                region: request.region.clone(),
                bucket: request.bucket.clone(),
                addressing: match request.addressing {
                    S3AddressingWire::Path => S3Addressing::Path,
                    S3AddressingWire::VirtualHosted => S3Addressing::VirtualHosted,
                },
                allow_http: false,
                // The facade owns its caller deadline. Leave a bounded margin
                // so the native transport timeout cannot race its typed outcome.
                operation_timeout: Duration::from_millis(u64::from(request.timeout_ms))
                    + Duration::from_secs(1),
            };
            match credentials {
                Some(c) => S3Reader::new_authenticated(config, c.into_native()?),
                None => S3Reader::new(config),
            }
            .map_err(|_| PublicError::invalid_params())
        };
        let reader = match prepare() {
            Ok(r) => r,
            Err(error) => return Ok(S3DiscoveryOutcome::Rejected { error }),
        };
        let mut state = self.0.lock().map_err(|_| unavailable())?;
        let Some(sender) = state.sender.as_ref().filter(|s| !s.is_closed()) else {
            return Ok(S3DiscoveryOutcome::Unavailable);
        };
        if state.current.as_ref().is_some_and(|c| c.result.is_none())
            || state
                .discovery
                .as_ref()
                .is_some_and(|d| d.result.is_none() || d.id == request.operation_id)
        {
            return Ok(S3DiscoveryOutcome::Rejected {
                error: PublicError {
                    code: -32009,
                    class: PublicErrorClass::Conflict,
                    message: "Observe the existing S3 operation before starting discovery.",
                },
            });
        }
        let (cancel, cancelled) = watch::channel(false);
        let id = request.operation_id;
        sender
            .try_send(Job::Discovery(DiscoveryJob {
                id: id.clone(),
                reader,
                prefix: request.prefix,
                deadline: Duration::from_millis(u64::from(request.timeout_ms)),
                cancelled,
            }))
            .map_err(|_| unavailable())?;
        state.discovery = Some(CurrentDiscovery {
            id: id.clone(),
            cancel,
            result: None,
        });
        Ok(S3DiscoveryOutcome::Running { operation_id: id })
    }
    pub(crate) fn discovery_snapshot(&self, id: Option<&str>) -> Result<S3DiscoveryOutcome> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        Ok(snapshot(&state, id))
    }
    pub(crate) fn cancel_discovery(&self, id: &str) -> Result<S3DiscoveryOutcome> {
        let state = self.0.lock().map_err(|_| unavailable())?;
        if let Some(d) = state
            .discovery
            .as_ref()
            .filter(|d| d.id == id && d.result.is_none())
        {
            d.cancel.send_replace(true);
        }
        // Cancellation is a request. Only worker completion publishes Cancelled.
        Ok(snapshot(&state, Some(id)))
    }
    pub(super) fn finish_discovery(&self, id: &str, result: S3DiscoveryOutcome) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(d) = state.discovery.as_mut().filter(|d| d.id == id) {
                d.result = Some(if *d.cancel.borrow() {
                    S3DiscoveryOutcome::Cancelled {
                        operation_id: id.into(),
                    }
                } else {
                    result
                });
            }
        }
    }
}
fn snapshot(state: &Inner, id: Option<&str>) -> S3DiscoveryOutcome {
    if state.sender.as_ref().is_none_or(mpsc::Sender::is_closed) {
        return S3DiscoveryOutcome::Unavailable;
    }
    let Some(d) = &state.discovery else {
        return id.map_or(S3DiscoveryOutcome::Idle, |id| {
            S3DiscoveryOutcome::NotFound {
                operation_id: id.into(),
            }
        });
    };
    if id.is_some_and(|id| id != d.id) {
        return S3DiscoveryOutcome::NotFound {
            operation_id: id.unwrap().into(),
        };
    }
    d.result
        .clone()
        .unwrap_or_else(|| S3DiscoveryOutcome::Running {
            operation_id: d.id.clone(),
        })
}
pub(super) async fn run(mut job: DiscoveryJob) -> S3DiscoveryOutcome {
    let id = job.id;
    let cancel = async {
        while !*job.cancelled.borrow_and_update() {
            if job.cancelled.changed().await.is_err() {
                break;
            }
        }
    };
    let listing = tokio::select! {
        biased;
        () = cancel => return S3DiscoveryOutcome::Cancelled { operation_id: id },
        result = tokio::time::timeout(job.deadline, job.reader.enumerate_prefix(&job.prefix, S3PrefixLimits {
            page_size: 16, max_pages: 8, max_objects: 32, max_page_bytes: 64 * 1024, max_total_bytes: 256 * 1024,
        })) => match result {
            Ok(result) => result,
            Err(_) => return S3DiscoveryOutcome::Deadline { operation_id: id },
        },
    };
    match listing {
        Ok(listing) => {
            // Wire projection is bounded independently of XML and object counts.
            let mut objects = Vec::with_capacity(listing.objects().len());
            for o in listing.objects() {
                if o.key().len() > 1024 || o.version().len() > 4096 || o.etag().len() > 1024 {
                    return S3DiscoveryOutcome::Incomplete { operation_id: id };
                }
                objects.push(S3DiscoveredObject {
                    key: o.key().into(),
                    version_id: o.version().into(),
                    etag: o.etag().into(),
                    size_bytes: o.size().to_string(),
                });
            }
            let outcome = S3DiscoveryOutcome::Complete {
                operation_id: id.clone(),
                objects,
                pages: listing.pages(),
            };
            if serde_json::to_vec(&outcome).map_or(true, |b| b.len() > 60 * 1024) {
                S3DiscoveryOutcome::Incomplete { operation_id: id }
            } else {
                outcome
            }
        }
        Err(S3PrefixError::Incomplete(_)) => S3DiscoveryOutcome::Incomplete { operation_id: id },
        Err(S3PrefixError::Reader(S3ReaderError::TimedOut)) => {
            S3DiscoveryOutcome::Deadline { operation_id: id }
        }
        Err(_) => S3DiscoveryOutcome::Unavailable,
    }
}
