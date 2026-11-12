//! Which ONNX Runtime CUDA builds can install, before anyone clicks install.
//!
//! A CUDA build installs only when Microsoft's metadata for it names CUDA
//! libraries that NVIDIA publishes for the host (see [`super::onnx_cuda`]).
//! The version listing asks this owner about every CUDA build it lists and
//! gives each one an explicit state: `checking` while its lookup runs,
//! `available`, `unavailable` with the reason (which names the builds that do
//! install), or `unknown` when the indexes could not be read, in which case an
//! install still decides. The answers come from the same resolution an install
//! runs, never from a table.
//!
//! One lookup runs per build and generation; listings that arrive while it
//! runs wait on the same lookup. An answer is kept until the package indexes
//! change or a refresh is asked for: it is tied to the [`IndexCache`]
//! generation, and a newer generation revalidates the index pages it read
//! (conditional requests the index answers with "not modified" when nothing
//! changed).
//!
//! [`IndexCache`]: super::python_index::IndexCache

use super::onnx_cuda::{self, CudaWheelSources};
use super::python_index::WheelPlatform;
use futures::StreamExt;
use pumas_library::models::{GitHubRelease, InstallAvailability};
use pumas_library::onnx_runtime_distribution::{release_version, OnnxRuntimeVariant};
use pumas_library::PumasError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::OnceCell;

/// Builds resolved at once while a listing is annotated.
const LISTING_CONCURRENCY: usize = 4;
/// How long a listing waits for answers. A lookup still running afterwards is
/// listed as `checking` and keeps running; the next listing waits on it.
const LISTING_DEADLINE: Duration = Duration::from_secs(10);

/// Whether a CUDA build installs, as the package indexes say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CudaBuildOutcome {
    Installs,
    /// The build cannot install, and why.
    Unavailable(String),
}

/// A finished lookup: the outcome, or `None` when the indexes could not be
/// read (the build's state is unknown).
type Answer = Option<CudaBuildOutcome>;

/// The lookup of one build in one index generation.
#[derive(Debug)]
struct Slot {
    generation: u64,
    answer: Arc<OnceCell<Answer>>,
}

/// The CUDA builds a listing offers and whether each installs.
#[derive(Debug)]
pub(crate) struct CudaAvailability {
    sources: CudaWheelSources,
    slots: StdMutex<HashMap<String, Slot>>,
    /// The CUDA build tags of the latest listing.
    listed: StdMutex<Vec<String>>,
    listing_deadline: Duration,
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
            slots: StdMutex::default(),
            listed: StdMutex::default(),
            listing_deadline: LISTING_DEADLINE,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_listing_deadline(mut self, deadline: Duration) -> Self {
        self.listing_deadline = deadline;
        self
    }

    pub(crate) fn sources(&self) -> &CudaWheelSources {
        &self.sources
    }

    /// Ask the indexes again: every kept answer is revalidated when next used.
    pub(crate) fn refresh(&self) {
        self.sources.cache.refresh();
    }

    /// The lookup of `tag` in the current generation, started or not.
    fn slot(&self, tag: &str) -> Arc<OnceCell<Answer>> {
        let generation = self.sources.cache.generation();
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = slots.entry(tag.to_owned()).or_insert_with(|| Slot {
            generation,
            answer: Arc::default(),
        });
        if slot.generation != generation {
            *slot = Slot {
                generation,
                answer: Arc::default(),
            };
        }
        slot.answer.clone()
    }

    /// The finished lookup of `tag` in the current generation, if any.
    fn finished(&self, tag: &str) -> Option<Answer> {
        self.slot(tag).get().cloned()
    }

    /// Keep what an install's own resolution found for `tag`.
    pub(crate) fn record(&self, tag: &str, outcome: CudaBuildOutcome) {
        let generation = self.sources.cache.generation();
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                tag.to_owned(),
                Slot {
                    generation,
                    answer: Arc::new(OnceCell::new_with(Some(Some(outcome)))),
                },
            );
    }

    /// Whether the CUDA build `tag` installs on this host, or `None` when the
    /// indexes could not say. Concurrent callers share one lookup.
    pub(crate) async fn outcome(&self, tag: &str) -> Answer {
        self.slot(tag)
            .get_or_init(|| self.look_up(tag))
            .await
            .clone()
    }

    async fn look_up(&self, tag: &str) -> Answer {
        let (release, major) = cuda_build(tag)?;
        let platform = WheelPlatform::host()?;
        match onnx_cuda::resolve(&self.sources, tag, &release, major, platform).await {
            Ok(_) => Some(CudaBuildOutcome::Installs),
            Err(error) => refusal_reason(&error).map(CudaBuildOutcome::Unavailable),
        }
    }

    /// The finished lookups of `tags`, after waiting at most
    /// [`LISTING_DEADLINE`] for those still running. A build missing from the
    /// result is still being checked; its lookup keeps running.
    async fn answers(self: &Arc<Self>, tags: &[String]) -> HashMap<String, Answer> {
        let owner = Arc::clone(self);
        let pending = tags.to_vec();
        let lookups = tokio::spawn(async move {
            futures::stream::iter(pending)
                .map(|tag| {
                    let owner = Arc::clone(&owner);
                    async move { owner.outcome(&tag).await }
                })
                .buffer_unordered(LISTING_CONCURRENCY)
                .collect::<Vec<_>>()
                .await
        });
        let _ = tokio::time::timeout(self.listing_deadline, lookups).await;
        tags.iter()
            .filter_map(|tag| self.finished(tag).map(|answer| (tag.clone(), answer)))
            .collect()
    }

    /// Give every listed CUDA build its install state, and a build that
    /// cannot install its reason.
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
        let answers = self.answers(&tags).await;
        let outcomes = known(&answers);
        for release in releases.iter_mut() {
            if cuda_build(&release.tag_name).is_none() {
                continue;
            }
            release.install_availability = Some(match answers.get(&release.tag_name) {
                None => InstallAvailability::Checking,
                Some(None) => InstallAvailability::Unknown,
                Some(Some(CudaBuildOutcome::Installs)) => InstallAvailability::Available,
                Some(Some(CudaBuildOutcome::Unavailable(reason))) => {
                    release.unavailable_reason =
                        Some(explain(&release.tag_name, reason, &tags, &outcomes));
                    InstallAvailability::Unavailable
                }
            });
        }
    }

    /// `reason` for refusing `tag`, followed by the listed builds that install.
    pub(crate) async fn explain(self: &Arc<Self>, tag: &str, reason: &str) -> String {
        let listed = self
            .listed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let answers = self.answers(&listed).await;
        explain(tag, reason, &listed, &known(&answers))
    }
}

/// The builds whose outcome is known.
fn known(answers: &HashMap<String, Answer>) -> HashMap<String, CudaBuildOutcome> {
    answers
        .iter()
        .filter_map(|(tag, answer)| answer.clone().map(|outcome| (tag.clone(), outcome)))
        .collect()
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
