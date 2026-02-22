//! Thread-safe progress tracking for conversion operations.

use std::collections::HashMap;
use std::sync::Mutex;

use super::types::{ConversionProgress, ConversionStatus, ScriptProgressLine};

/// Tracks progress of all active and recently completed conversions.
pub struct ConversionProgressTracker {
    state: Mutex<HashMap<String, ConversionProgress>>,
}

impl ConversionProgressTracker {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Insert a new conversion progress entry.
    pub fn insert(&self, progress: ConversionProgress) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        state.insert(progress.conversion_id.clone(), progress);
    }

    /// Get progress for a specific conversion.
    pub fn get(&self, conversion_id: &str) -> Option<ConversionProgress> {
        let state = self.state.lock().expect("progress lock poisoned");
        state.get(conversion_id).cloned()
    }

    /// List all tracked conversions.
    pub fn list_all(&self) -> Vec<ConversionProgress> {
        let state = self.state.lock().expect("progress lock poisoned");
        state.values().cloned().collect()
    }

    /// Update progress from a Python script stdout JSON line.
    pub fn update_from_script(&self, conversion_id: &str, line: &ScriptProgressLine) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        let Some(progress) = state.get_mut(conversion_id) else {
            return;
        };

        match line.stage.as_str() {
            "validating" => {
                progress.status = ConversionStatus::Validating;
            }
            "converting" => {
                progress.status = ConversionStatus::Converting;
                progress.current_tensor = line.tensor_name.clone();
                progress.tensors_completed = line.tensor_index;
                progress.tensors_total = line.tensor_count;
                progress.bytes_written = line.bytes_written;

                // Compute overall progress from tensor counts
                if let (Some(done), Some(total)) = (line.tensor_index, line.tensor_count) {
                    if total > 0 {
                        progress.progress = Some(done as f32 / total as f32);
                    }
                }
            }
            "writing" => {
                progress.status = ConversionStatus::Writing;
                progress.progress = Some(0.95);
            }
            "complete" => {
                progress.status = ConversionStatus::Completed;
                progress.progress = Some(1.0);
                if let Some(size) = line.output_size {
                    progress.estimated_output_size = Some(size);
                }
            }
            "error" => {
                progress.status = ConversionStatus::Error;
                progress.error = line.message.clone();
            }
            _ => {}
        }
    }

    /// Update the status of a conversion directly.
    pub fn set_status(&self, conversion_id: &str, status: ConversionStatus) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        if let Some(progress) = state.get_mut(conversion_id) {
            progress.status = status;
        }
    }

    /// Set the error message for a conversion.
    pub fn set_error(&self, conversion_id: &str, message: String) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        if let Some(progress) = state.get_mut(conversion_id) {
            progress.status = ConversionStatus::Error;
            progress.error = Some(message);
        }
    }

    /// Set the output model ID after successful import.
    pub fn set_output_model_id(&self, conversion_id: &str, model_id: String) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        if let Some(progress) = state.get_mut(conversion_id) {
            progress.output_model_id = Some(model_id);
        }
    }

    /// Remove a conversion entry.
    pub fn remove(&self, conversion_id: &str) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        state.remove(conversion_id);
    }

    /// Update pipeline step information for multi-step operations.
    ///
    /// Resets per-step progress fields (progress, tensor counts, current tensor)
    /// when transitioning between pipeline steps.
    pub fn update_pipeline(
        &self,
        conversion_id: &str,
        step: u32,
        total: u32,
        label: &str,
    ) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        if let Some(progress) = state.get_mut(conversion_id) {
            progress.pipeline_step = Some(step);
            progress.pipeline_steps_total = Some(total);
            progress.pipeline_step_label = Some(label.to_string());
            // Reset per-step progress when entering a new step.
            progress.progress = Some(0.0);
            progress.tensors_completed = None;
            progress.tensors_total = None;
            progress.current_tensor = None;
        }
    }

    /// Update tensor-level progress (used by llama-quantize output parser).
    pub fn update_tensor_progress(
        &self,
        conversion_id: &str,
        completed: u32,
        total: u32,
        tensor_name: &str,
    ) {
        let mut state = self.state.lock().expect("progress lock poisoned");
        if let Some(progress) = state.get_mut(conversion_id) {
            progress.tensors_completed = Some(completed);
            progress.tensors_total = Some(total);
            progress.current_tensor = Some(tensor_name.to_string());
            if total > 0 {
                progress.progress = Some(completed as f32 / total as f32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::ConversionDirection;

    fn make_progress(id: &str) -> ConversionProgress {
        ConversionProgress {
            conversion_id: id.to_string(),
            source_model_id: "llm/llama/test-model".to_string(),
            direction: ConversionDirection::GgufToSafetensors,
            status: ConversionStatus::SettingUp,
            progress: None,
            current_tensor: None,
            tensors_completed: None,
            tensors_total: None,
            bytes_written: None,
            estimated_output_size: None,
            target_quant: None,
            error: None,
            output_model_id: None,
            pipeline_step: None,
            pipeline_steps_total: None,
            pipeline_step_label: None,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let tracker = ConversionProgressTracker::new();
        let progress = make_progress("conv-1");
        tracker.insert(progress);

        let result = tracker.get("conv-1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().conversion_id, "conv-1");
    }

    #[test]
    fn test_update_from_script_converting() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));

        let line = ScriptProgressLine {
            stage: "converting".to_string(),
            tensor_index: Some(10),
            tensor_count: Some(100),
            tensor_name: Some("model.layers.5.self_attn.q_proj.weight".to_string()),
            bytes_written: Some(1024),
            output_path: None,
            output_size: None,
            message: None,
        };

        tracker.update_from_script("conv-1", &line);
        let progress = tracker.get("conv-1").unwrap();

        assert_eq!(progress.status, ConversionStatus::Converting);
        assert_eq!(progress.tensors_completed, Some(10));
        assert_eq!(progress.tensors_total, Some(100));
        assert!((progress.progress.unwrap() - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_update_from_script_complete() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));

        let line = ScriptProgressLine {
            stage: "complete".to_string(),
            tensor_index: None,
            tensor_count: None,
            tensor_name: None,
            bytes_written: None,
            output_path: Some("/output/model.safetensors".to_string()),
            output_size: Some(14_000_000_000),
            message: None,
        };

        tracker.update_from_script("conv-1", &line);
        let progress = tracker.get("conv-1").unwrap();

        assert_eq!(progress.status, ConversionStatus::Completed);
        assert_eq!(progress.progress, Some(1.0));
        assert_eq!(progress.estimated_output_size, Some(14_000_000_000));
    }

    #[test]
    fn test_set_error() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));

        tracker.set_error("conv-1", "Python crashed".to_string());
        let progress = tracker.get("conv-1").unwrap();

        assert_eq!(progress.status, ConversionStatus::Error);
        assert_eq!(progress.error.as_deref(), Some("Python crashed"));
    }

    #[test]
    fn test_list_all() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));
        tracker.insert(make_progress("conv-2"));

        let all = tracker.list_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_remove() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));
        tracker.remove("conv-1");

        assert!(tracker.get("conv-1").is_none());
    }

    #[test]
    fn test_update_pipeline_resets_per_step_fields() {
        let tracker = ConversionProgressTracker::new();
        let mut p = make_progress("conv-1");
        p.tensors_completed = Some(50);
        p.tensors_total = Some(100);
        p.current_tensor = Some("layer.5.weight".to_string());
        p.progress = Some(0.5);
        tracker.insert(p);

        tracker.update_pipeline("conv-1", 2, 3, "Quantizing to Q4_K_M");
        let result = tracker.get("conv-1").unwrap();

        assert_eq!(result.pipeline_step, Some(2));
        assert_eq!(result.pipeline_steps_total, Some(3));
        assert_eq!(result.pipeline_step_label.as_deref(), Some("Quantizing to Q4_K_M"));
        // Per-step fields should be reset
        assert_eq!(result.progress, Some(0.0));
        assert_eq!(result.tensors_completed, None);
        assert_eq!(result.tensors_total, None);
        assert_eq!(result.current_tensor, None);
    }

    #[test]
    fn test_update_tensor_progress() {
        let tracker = ConversionProgressTracker::new();
        tracker.insert(make_progress("conv-1"));

        tracker.update_tensor_progress("conv-1", 25, 100, "model.layers.10.attn_k.weight");
        let result = tracker.get("conv-1").unwrap();

        assert_eq!(result.tensors_completed, Some(25));
        assert_eq!(result.tensors_total, Some(100));
        assert_eq!(
            result.current_tensor.as_deref(),
            Some("model.layers.10.attn_k.weight")
        );
        assert!((result.progress.unwrap() - 0.25).abs() < 0.01);
    }
}
