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
}
