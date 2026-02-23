//! GPU monitoring support.
//!
//! Provides GPU monitoring capabilities for NVIDIA GPUs via nvidia-smi.
//! Future support for AMD ROCm and Intel GPUs can be added.

use crate::error::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::debug;

/// GPU information.
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    /// GPU utilization percentage (0-100).
    pub usage: f32,
    /// Used GPU memory in bytes.
    pub memory_used: u64,
    /// Total GPU memory in bytes.
    pub memory_total: u64,
    /// GPU temperature in Celsius (if available).
    pub temperature: Option<f32>,
    /// GPU name/model.
    pub name: Option<String>,
}

/// Trait for GPU monitoring implementations.
pub trait GpuMonitor: Send + Sync {
    /// Check if GPU monitoring is available.
    fn is_available(&self) -> bool;

    /// Get overall GPU information.
    fn get_gpu_info(&self) -> Result<GpuInfo>;

    /// Get GPU memory usage for a specific process.
    fn get_process_gpu_memory(&self, pid: u32) -> Result<u64>;

    /// Get GPU memory usage for multiple processes.
    fn get_processes_gpu_memory(&self, pids: &[u32]) -> Result<HashMap<u32, u64>>;

    /// Refresh cached data.
    fn refresh(&self);
}

/// NVIDIA GPU monitor using nvidia-smi.
pub struct NvidiaSmiMonitor {
    /// Cache TTL in seconds.
    cache_ttl: Duration,
    /// Cached GPU info.
    gpu_cache: Arc<RwLock<Option<(GpuInfo, Instant)>>>,
    /// Cached per-process GPU memory.
    process_cache: Arc<RwLock<Option<(HashMap<u32, u64>, Instant)>>>,
    /// Whether nvidia-smi is available.
    available: bool,
}

impl NvidiaSmiMonitor {
    /// Create a new NVIDIA GPU monitor.
    ///
    /// # Arguments
    ///
    /// * `cache_ttl` - How long to cache nvidia-smi results (default: 2 seconds)
    pub fn new(cache_ttl: Duration) -> Self {
        // Check if nvidia-smi is available
        let available = Command::new("nvidia-smi")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !available {
            debug!("nvidia-smi not available - GPU monitoring disabled");
        }

        Self {
            cache_ttl,
            gpu_cache: Arc::new(RwLock::new(None)),
            process_cache: Arc::new(RwLock::new(None)),
            available,
        }
    }

    /// Create with default settings (2 second cache TTL).
    pub fn default() -> Self {
        Self::new(Duration::from_secs(2))
    }

    /// Query nvidia-smi for GPU utilization and memory.
    fn query_gpu_info(&self) -> Result<GpuInfo> {
        if !self.available {
            return Ok(GpuInfo::default());
        }

        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,name",
                "--format=csv,noheader,nounits",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let line = stdout.lines().next().unwrap_or("");
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

                if parts.len() >= 4 {
                    let usage = parts[0].parse::<f32>().unwrap_or(0.0);
                    let memory_used_mb = parts[1].parse::<u64>().unwrap_or(0);
                    let memory_total_mb = parts[2].parse::<u64>().unwrap_or(0);
                    let temperature = parts[3].parse::<f32>().ok();
                    let name = parts.get(4).map(|s| s.to_string());

                    Ok(GpuInfo {
                        usage,
                        memory_used: memory_used_mb * 1024 * 1024,
                        memory_total: memory_total_mb * 1024 * 1024,
                        temperature,
                        name,
                    })
                } else {
                    debug!("Unexpected nvidia-smi output format: {}", line);
                    Ok(GpuInfo::default())
                }
            }
            Ok(output) => {
                debug!(
                    "nvidia-smi returned non-zero: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                Ok(GpuInfo::default())
            }
            Err(e) => {
                debug!("Failed to run nvidia-smi: {}", e);
                Ok(GpuInfo::default())
            }
        }
    }

    /// Query nvidia-smi for per-process GPU memory.
    fn query_process_gpu_memory(&self) -> Result<HashMap<u32, u64>> {
        if !self.available {
            return Ok(HashMap::new());
        }

        let output = Command::new("nvidia-smi")
            .args([
                "--query-compute-apps=pid,used_memory",
                "--format=csv,noheader,nounits",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut processes = HashMap::new();

                for line in stdout.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 2 {
                        if let (Ok(pid), Ok(memory_mb)) =
                            (parts[0].parse::<u32>(), parts[1].parse::<u64>())
                        {
                            // Convert MB to bytes
                            processes.insert(pid, memory_mb * 1024 * 1024);
                        }
                    }
                }

                Ok(processes)
            }
            Ok(output) => {
                debug!(
                    "nvidia-smi process query returned non-zero: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                Ok(HashMap::new())
            }
            Err(e) => {
                debug!("Failed to run nvidia-smi for process query: {}", e);
                Ok(HashMap::new())
            }
        }
    }
}

impl GpuMonitor for NvidiaSmiMonitor {
    fn is_available(&self) -> bool {
        self.available
    }

    fn get_gpu_info(&self) -> Result<GpuInfo> {
        // Check cache first
        {
            let cache = self.gpu_cache.read().unwrap();
            if let Some((info, timestamp)) = cache.as_ref() {
                if timestamp.elapsed() < self.cache_ttl {
                    return Ok(info.clone());
                }
            }
        }

        // Cache miss - query nvidia-smi
        let info = self.query_gpu_info()?;

        // Update cache
        {
            let mut cache = self.gpu_cache.write().unwrap();
            *cache = Some((info.clone(), Instant::now()));
        }

        Ok(info)
    }

    fn get_process_gpu_memory(&self, pid: u32) -> Result<u64> {
        let processes = self.get_processes_gpu_memory(&[pid])?;
        Ok(processes.get(&pid).copied().unwrap_or(0))
    }

    fn get_processes_gpu_memory(&self, pids: &[u32]) -> Result<HashMap<u32, u64>> {
        // Check cache first
        {
            let cache = self.process_cache.read().unwrap();
            if let Some((all_processes, timestamp)) = cache.as_ref() {
                if timestamp.elapsed() < self.cache_ttl {
                    // Filter to only requested PIDs
                    let filtered: HashMap<u32, u64> = pids
                        .iter()
                        .filter_map(|pid| all_processes.get(pid).map(|mem| (*pid, *mem)))
                        .collect();
                    return Ok(filtered);
                }
            }
        }

        // Cache miss - query nvidia-smi
        let all_processes = self.query_process_gpu_memory()?;

        // Update cache
        {
            let mut cache = self.process_cache.write().unwrap();
            *cache = Some((all_processes.clone(), Instant::now()));
        }

        // Filter to only requested PIDs
        let filtered: HashMap<u32, u64> = pids
            .iter()
            .filter_map(|pid| all_processes.get(pid).map(|mem| (*pid, *mem)))
            .collect();

        Ok(filtered)
    }

    fn refresh(&self) {
        // Clear caches to force refresh on next query
        {
            let mut cache = self.gpu_cache.write().unwrap();
            *cache = None;
        }
        {
            let mut cache = self.process_cache.write().unwrap();
            *cache = None;
        }
    }
}

/// No-op GPU monitor for systems without GPU support.
pub struct NoOpGpuMonitor;

impl GpuMonitor for NoOpGpuMonitor {
    fn is_available(&self) -> bool {
        false
    }

    fn get_gpu_info(&self) -> Result<GpuInfo> {
        Ok(GpuInfo::default())
    }

    fn get_process_gpu_memory(&self, _pid: u32) -> Result<u64> {
        Ok(0)
    }

    fn get_processes_gpu_memory(&self, pids: &[u32]) -> Result<HashMap<u32, u64>> {
        Ok(pids.iter().map(|&pid| (pid, 0u64)).collect())
    }

    fn refresh(&self) {}
}

/// Create the appropriate GPU monitor for the current system.
pub fn create_gpu_monitor() -> Box<dyn GpuMonitor> {
    let nvidia_monitor = NvidiaSmiMonitor::default();
    if nvidia_monitor.is_available() {
        Box::new(nvidia_monitor)
    } else {
        Box::new(NoOpGpuMonitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_op_monitor() {
        let monitor = NoOpGpuMonitor;

        assert!(!monitor.is_available());

        let info = monitor.get_gpu_info().unwrap();
        assert_eq!(info.usage, 0.0);
        assert_eq!(info.memory_used, 0);

        let mem = monitor.get_process_gpu_memory(1234).unwrap();
        assert_eq!(mem, 0);
    }

    #[test]
    fn test_nvidia_monitor_creation() {
        // This test will pass regardless of whether nvidia-smi is available
        let monitor = NvidiaSmiMonitor::default();

        // Should not panic
        let _ = monitor.get_gpu_info();
        let _ = monitor.get_process_gpu_memory(1234);
    }
}
