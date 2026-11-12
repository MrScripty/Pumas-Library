//! Which ONNX Runtime CUDA builds can install, before anyone clicks install.
//!
//! A CUDA build installs only when Microsoft's metadata for it names CUDA
//! libraries that NVIDIA publishes for the host (see [`super::onnx_cuda`]).
//! The version listing asks this owner about every CUDA build it lists, so a
//! build that cannot install is reported with its reason, and the reason names
//! the builds that do install. The answers come from the same resolution an
//! install runs, never from a table.
//!
//! An answer is kept per build until the package indexes change or a refresh
//! is asked for: it is tied to the [`IndexCache`] generation, and a newer
//! generation revalidates the index pages it read (conditional requests the
//! index answers with "not modified" when nothing changed). An answer the
//! indexes could not give, such as a network failure, is not kept and does not
//! mark a build unavailable.

use super::onnx_cuda::{self, CudaWheelSources};
use super::python_index::WheelPlatform;
use futures::StreamExt;
use pumas_library::models::GitHubRelease;
use pumas_library::onnx_runtime_distribution::{release_version, OnnxRuntimeVariant};
use pumas_library::PumasError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

/// Builds resolved at once while a listing is annotated.
const LISTING_CONCURRENCY: usize = 4;
/// How long a listing waits for answers. Lookups still running afterwards
/// finish in the background and answer the next listing; until then the build
/// is listed without a reason, and an install of it still checks.
const LISTING_DEADLINE: Duration = Duration::from_secs(10);

/// Whether a CUDA build installs, as the package indexes say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CudaBuildOutcome {
    Installs,
    /// The build cannot install, and why.
    Unavailable(String),
}

#[derive(Clone)]
struct Entry {
    generation: u64,
    outcome: CudaBuildOutcome,
}

/// The CUDA builds a listing offers and whether each installs.
#[derive(Debug)]
pub(crate) struct CudaAvailability {
    sources: CudaWheelSources,
    entries: StdMutex<HashMap<String, Entry>>,
    /// The CUDA build tags of the latest listing.
    listed: StdMutex<Vec<String>>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Entry")
            .field("generation", &self.generation)
            .field("outcome", &self.outcome)
            .finish()
    }
}

/// The message of a resolution refusal, or `None` when the failure says
/// nothing about the build (the indexes could not be read).
pub(crate) fn refusal_reason(error: &PumasError) -> Option<String> {
    match error {
        PumasError::Network { .. } | PumasError::RateLimited { .. } => None,
        PumasError::InstallationFailed { message } => Some(message.clone()),
        other => Some(other.to_string()),
    }
}

fn cuda_build(tag: &str) -> Option<(semver::Version, u32)> {
    let major = OnnxRuntimeVariant::from_tag(tag)?.cuda_major()?;
    Some((release_version(tag)?, major))
}

impl CudaAvailability {
    pub(crate) fn new(sources: CudaWheelSources) -> Self {
        Self {
            sources,
            entries: StdMutex::default(),
            listed: StdMutex::default(),
        }
    }

    pub(crate) fn sources(&self) -> &CudaWheelSources {
        &self.sources
    }

    /// Ask the indexes again: every kept answer is revalidated when next used.
    pub(crate) fn refresh(&self) {
        self.sources.cache.refresh();
    }

    fn kept(&self, tag: &str) -> Option<CudaBuildOutcome> {
        let generation = self.sources.cache.generation();
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(tag)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.outcome.clone())
    }

    /// Keep what an install's own resolution found for `tag`.
    pub(crate) fn record(&self, tag: &str, outcome: CudaBuildOutcome) {
        let generation = self.sources.cache.generation();
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                tag.to_owned(),
                Entry {
                    generation,
                    outcome,
                },
            );
    }

    /// Whether the CUDA build `tag` installs on this host, or `None` when the
    /// indexes could not say.
    pub(crate) async fn outcome(&self, tag: &str) -> Option<CudaBuildOutcome> {
        if let Some(outcome) = self.kept(tag) {
            return Some(outcome);
        }
        let (release, major) = cuda_build(tag)?;
        let platform = WheelPlatform::host()?;
        let generation = self.sources.cache.generation();
        let outcome = match onnx_cuda::resolve(&self.sources, tag, &release, major, platform).await
        {
            Ok(_) => CudaBuildOutcome::Installs,
            Err(error) => CudaBuildOutcome::Unavailable(refusal_reason(&error)?),
        };
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                tag.to_owned(),
                Entry {
                    generation,
                    outcome: outcome.clone(),
                },
            );
        Some(outcome)
    }

    /// The answers for `tags`, waiting at most [`LISTING_DEADLINE`] for those
    /// not known yet. A lookup that takes longer keeps running and keeps its
    /// answer for the next caller.
    async fn outcomes(self: &Arc<Self>, tags: &[String]) -> HashMap<String, CudaBuildOutcome> {
        let owner = Arc::clone(self);
        let pending = tags.to_vec();
        let lookups = tokio::spawn(async move {
            futures::stream::iter(pending)
                .map(|tag| {
                    let owner = Arc::clone(&owner);
                    async move {
                        let outcome = owner.outcome(&tag).await;
                        (tag, outcome)
                    }
                })
                .buffer_unordered(LISTING_CONCURRENCY)
                .filter_map(|(tag, outcome)| async move { outcome.map(|outcome| (tag, outcome)) })
                .collect::<HashMap<_, _>>()
                .await
        });
        match tokio::time::timeout(LISTING_DEADLINE, lookups).await {
            Ok(Ok(outcomes)) => outcomes,
            _ => tags
                .iter()
                .filter_map(|tag| self.kept(tag).map(|outcome| (tag.clone(), outcome)))
                .collect(),
        }
    }

    /// Mark every listed CUDA build that cannot install with its reason.
    pub(crate) async fn annotate(self: &Arc<Self>, releases: &mut [GitHubRelease]) {
        let tags: Vec<String> = releases
            .iter()
            .map(|release| release.tag_name.clone())
            .filter(|tag| cuda_build(tag).is_some())
            .collect();
        *self
            .listed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = tags.clone();
        let outcomes = self.outcomes(&tags).await;
        for release in releases.iter_mut() {
            if let Some(CudaBuildOutcome::Unavailable(reason)) = outcomes.get(&release.tag_name) {
                release.unavailable_reason =
                    Some(explain(&release.tag_name, reason, &tags, &outcomes));
            }
        }
    }

    /// `reason` for refusing `tag`, followed by the listed builds that install.
    pub(crate) async fn explain(self: &Arc<Self>, tag: &str, reason: &str) -> String {
        let listed = self
            .listed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let outcomes = self.outcomes(&listed).await;
        explain(tag, reason, &listed, &outcomes)
    }
}

/// The installing builds of CUDA `major` among `listed`, newest first, and
/// whether every listed build from the oldest of them on installs.
fn installing(
    major: u32,
    listed: &[String],
    outcomes: &HashMap<String, CudaBuildOutcome>,
) -> (Vec<(semver::Version, String)>, bool) {
    let mut builds: Vec<(semver::Version, String, Option<bool>)> = listed
        .iter()
        .filter_map(|tag| {
            let (version, build_major) = cuda_build(tag)?;
            (build_major == major).then(|| {
                let installs = outcomes
                    .get(tag)
                    .map(|outcome| *outcome == CudaBuildOutcome::Installs);
                (version, tag.clone(), installs)
            })
        })
        .collect();
    builds.sort_by(|left, right| right.0.cmp(&left.0));
    let installs: Vec<_> = builds
        .iter()
        .filter(|(_, _, installs)| *installs == Some(true))
        .map(|(version, tag, _)| (version.clone(), tag.clone()))
        .collect();
    let contiguous = match (installs.first(), installs.last()) {
        (Some((newest, _)), Some((oldest, _))) => builds
            .iter()
            .filter(|(version, _, _)| version >= oldest && version <= newest)
            .all(|(_, _, installs)| *installs == Some(true)),
        _ => false,
    };
    (installs, contiguous)
}

fn describe_major(
    major: u32,
    exclude: &str,
    listed: &[String],
    outcomes: &HashMap<String, CudaBuildOutcome>,
) -> String {
    let (installs, contiguous) = installing(major, listed, outcomes);
    let installs: Vec<_> = installs
        .into_iter()
        .filter(|(_, tag)| tag != exclude)
        .collect();
    match installs.as_slice() {
        [] => format!("no listed CUDA {major} build installs"),
        [(_, only)] => format!("{only} installs"),
        [(_, newest), .., (oldest, _)] if contiguous => {
            format!("+cuda{major} installs from {oldest} (newest {newest})")
        }
        [(_, newest), ..] => format!("{newest} is the newest CUDA {major} build that installs"),
    }
}

/// `reason`, then the builds of the same CUDA major and of the other one that
/// install, from what is known about `listed`.
fn explain(
    tag: &str,
    reason: &str,
    listed: &[String],
    outcomes: &HashMap<String, CudaBuildOutcome>,
) -> String {
    let Some((_, major)) = cuda_build(tag) else {
        return reason.to_owned();
    };
    let majors: Vec<u32> = OnnxRuntimeVariant::ALL
        .iter()
        .filter_map(|variant| variant.cuda_major())
        .collect();
    let mut parts = vec![describe_major(major, tag, listed, outcomes)];
    parts.extend(
        majors
            .into_iter()
            .filter(|other| *other != major)
            .map(|other| describe_major(other, tag, listed, outcomes)),
    );
    format!(
        "{reason}. Builds that install instead: {}.",
        parts.join("; ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcomes(entries: &[(&str, bool)]) -> (Vec<String>, HashMap<String, CudaBuildOutcome>) {
        let listed = entries.iter().map(|(tag, _)| (*tag).to_owned()).collect();
        let outcomes = entries
            .iter()
            .map(|(tag, installs)| {
                let outcome = if *installs {
                    CudaBuildOutcome::Installs
                } else {
                    CudaBuildOutcome::Unavailable("refused".into())
                };
                ((*tag).to_owned(), outcome)
            })
            .collect();
        (listed, outcomes)
    }

    #[test]
    fn a_refusal_names_the_builds_that_install_for_both_majors() {
        let (listed, known) = outcomes(&[
            ("v1.31.1+cuda12", false),
            ("v1.30.0+cuda12", false),
            ("v1.29.0+cuda12", true),
            ("v1.28.0+cuda12", true),
            ("v1.31.1+cuda13", true),
            ("v1.30.0+cuda13", true),
            ("v1.28.0+cuda13", true),
            ("v1.27.0+cuda13", false),
            ("v1.31.1+cpu", true),
        ]);
        let text = explain("v1.31.1+cuda12", "No metadata", &listed, &known);
        assert_eq!(
            text,
            "No metadata. Builds that install instead: +cuda12 installs from 1.28.0 (newest \
             v1.29.0+cuda12); +cuda13 installs from 1.28.0 (newest v1.31.1+cuda13)."
        );
        let text = explain("v1.27.0+cuda13", "Unsatisfiable", &listed, &known);
        assert!(
            text.contains("+cuda13 installs from 1.28.0 (newest v1.31.1+cuda13)"),
            "{text}"
        );
        assert!(text.contains("+cuda12 installs from 1.28.0"), "{text}");
    }

    #[test]
    fn gaps_and_absences_are_said_as_they_are() {
        let (listed, known) = outcomes(&[
            ("v1.31.1+cuda12", false),
            ("v1.30.0+cuda12", true),
            ("v1.29.0+cuda12", false),
            ("v1.28.0+cuda12", true),
            ("v1.31.1+cuda13", false),
        ]);
        let text = explain("v1.31.1+cuda12", "Refused", &listed, &known);
        assert!(
            text.contains("v1.30.0+cuda12 is the newest CUDA 12 build that installs"),
            "{text}"
        );
        assert!(text.contains("no listed CUDA 13 build installs"), "{text}");
        let (listed, known) = outcomes(&[("v1.31.1+cuda12", false), ("v1.29.0+cuda12", true)]);
        assert!(explain("v1.31.1+cuda12", "Refused", &listed, &known)
            .contains("v1.29.0+cuda12 installs"));
    }
}
