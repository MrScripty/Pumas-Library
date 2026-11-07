//! Native decision models served by llama.cpp.
//!
//! llama.cpp reports `decisions` in a model's `architecture.output_modalities`
//! (from GGUF metadata, without loading it). That runtime fact, read with the
//! model's other modalities, decides whether the model answers
//! `/v1/systemone`; prompted classifiers do not get it.

/// Library tasks under which decision models are published. While the
/// runtime cannot be asked, a model with one of these tasks reports
/// `decision` as waiting for its runtime rather than as a task mismatch.
pub(super) fn candidate_task(task: Option<&str>) -> bool {
    matches!(
        task,
        Some(
            "text-classification"
                | "zero-shot-classification"
                | "image-text-to-text"
                | "text+image->text"
                | "audio-text-to-text"
                | "any-to-any"
        )
    )
}
