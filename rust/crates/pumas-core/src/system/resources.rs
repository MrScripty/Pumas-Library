//! System and process resource tracking.
//!
//! Provides resource monitoring for:
//! - System-wide CPU, RAM, and disk usage
//! - Per-process CPU, RAM, and GPU memory usage

use super::gpu::{create_gpu_monitor, GpuMonitor};
use crate::error::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// System resource snapshot.
#[derive(Debug, Clone, Default)]
pub struct SystemResourceSnapshot {
    /// CPU usage percentage (0-100).
    pub cpu_usage: f32,
    /// CPU temperature in Celsius (if available).
    pub cpu_temp: Option<f32>,
    /// GPU usage percentage (0-100).
    pub gpu_usage: f32,
    /// GPU memory used in bytes.
    pub gpu_memory_used: u64,
    /// GPU memory total in bytes.
    pub gpu_memory_total: u64,
    /// GPU temperature in Celsius (if available).
    pub gpu_temp: Option<f32>,
    /// RAM used in bytes.
    pub ram_used: u64,
    /// RAM total in bytes.
    pub ram_total: u64,
    /// Disk used in bytes.
    pub disk_used: u64,
    /// Disk total in bytes.
    pub disk_total: u64,
    /// Disk free in bytes.
    pub disk_free: u64,
}

/// Per-process resource usage.
#[derive(Debug, Clone, Default)]
pub struct ProcessResources {
    /// CPU usage percentage (0-100+, can exceed 100 on multi-core).
    pub cpu: f32,
    /// RAM memory usage in GB.
    pub ram_memory: f32,
    /// GPU memory usage in GB.
    pub gpu_memory: f32,
}

/// Cached process resources.
struct CachedProcessResources {
    resources: ProcessResources,
    timestamp: Instant,
}

/// Resource tracker for monitoring system and process resources.
pub struct ResourceTracker {
    /// Cache TTL in seconds.
    cache_ttl: Duration,
    /// System info instance.
    system: Arc<RwLock<System>>,
    /// GPU monitor.
    gpu_monitor: Box<dyn GpuMonitor>,
    /// Per-process resource cache.
    process_cache: Arc<RwLock<HashMap<u32, CachedProcessResources>>>,
    /// Last system refresh time.
    last_system_refresh: Arc<RwLock<Option<Instant>>>,
}

impl ResourceTracker {
    /// Create a new resource tracker.
    ///
    /// # Arguments
    ///
    /// * `cache_ttl` - How long to cache resource measurements
    pub fn new(cache_ttl: Duration) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            cache_ttl,
            system: Arc::new(RwLock::new(system)),
            gpu_monitor: create_gpu_monitor(),
            process_cache: Arc::new(RwLock::new(HashMap::new())),
            last_system_refresh: Arc::new(RwLock::new(Some(Instant::now()))),
        }
    }

    /// Get a snapshot of system resources.
    pub fn get_system_resources(&self) -> Result<SystemResourceSnapshot> {
        // Refresh system info if needed
        self.maybe_refresh_system();

        let system = self.system.read().unwrap();

        // CPU
        let cpu_usage = system.global_cpu_usage();

        // RAM
        let ram_total = system.total_memory();
        let ram_used = system.used_memory();

        // Disk - use first disk as approximation
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (disk_total, disk_free) = if let Some(disk) = disks.list().first() {
            (disk.total_space(), disk.available_space())
        } else {
            (0, 0)
        };
        let disk_used = disk_total.saturating_sub(disk_free);

        // GPU
        let gpu_info = self.gpu_monitor.get_gpu_info()?;

        Ok(SystemResourceSnapshot {
            cpu_usage,
            cpu_temp: None, // Would need platform-specific implementation
            gpu_usage: gpu_info.usage,
            gpu_memory_used: gpu_info.memory_used,
            gpu_memory_total: gpu_info.memory_total,
            gpu_temp: gpu_info.temperature,
            ram_used,
            ram_total,
            disk_used,
            disk_total,
            disk_free,
        })
    }

    /// Get resource usage for a specific process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID
    /// * `include_children` - Whether to aggregate resources from child processes
    pub fn get_process_resources(
        &self,
        pid: u32,
        include_children: bool,
    ) -> Result<ProcessResources> {
        // Check cache first
        {
            let cache = self.process_cache.read().unwrap();
            if let Some(cached) = cache.get(&pid) {
                if cached.timestamp.elapsed() < self.cache_ttl {
                    return Ok(cached.resources.clone());
                }
            }
        }

        // Cache miss - compute resources
        let resources = self.compute_process_resources(pid, include_children)?;

        // Update cache
        {
            let mut cache = self.process_cache.write().unwrap();
            cache.insert(
                pid,
                CachedProcessResources {
                    resources: resources.clone(),
                    timestamp: Instant::now(),
                },
            );
        }

        Ok(resources)
    }

    /// Recursively collect all descendant PIDs of a process.
    fn collect_all_descendants(system: &sysinfo::System, parent_pid: Pid, pids: &mut Vec<u32>) {
        for (child_pid, child_process) in system.processes() {
            if child_process.parent() == Some(parent_pid) {
                let child_pid_u32 = child_pid.as_u32();
                if !pids.contains(&child_pid_u32) {
                    pids.push(child_pid_u32);
                    // Recursively find this child's children
                    Self::collect_all_descendants(system, *child_pid, pids);
                }
            }
        }
    }

    /// Compute resource usage for a process (and optionally its descendants).
    fn compute_process_resources(
        &self,
        pid: u32,
        include_children: bool,
    ) -> Result<ProcessResources> {
        self.maybe_refresh_system();

        let system = self.system.read().unwrap();
        let sysinfo_pid = Pid::from_u32(pid);

        // Collect PIDs to aggregate
        let mut pids_to_check = vec![pid];

        if include_children {
            // Recursively find ALL descendants, not just direct children
            Self::collect_all_descendants(&system, sysinfo_pid, &mut pids_to_check);
        }

        // Aggregate CPU and RAM
        let mut total_cpu = 0.0f32;
        let mut total_ram_bytes = 0u64;

        for &check_pid in &pids_to_check {
            let check_sysinfo_pid = Pid::from_u32(check_pid);
            if let Some(process) = system.process(check_sysinfo_pid) {
                total_cpu += process.cpu_usage();
                total_ram_bytes += process.memory();
            }
        }

        // Get GPU memory for all PIDs
        let gpu_memory_map = self.gpu_monitor.get_processes_gpu_memory(&pids_to_check)?;
        let total_gpu_bytes: u64 = gpu_memory_map.values().sum();

        Ok(ProcessResources {
            cpu: (total_cpu * 10.0).round() / 10.0, // Round to 1 decimal
            ram_memory: ((total_ram_bytes as f32 / 1_073_741_824.0) * 100.0).round() / 100.0, // GB, 2 decimals
            gpu_memory: ((total_gpu_bytes as f32 / 1_073_741_824.0) * 100.0).round() / 100.0, // GB, 2 decimals
        })
    }

    /// Clear the process resource cache.
    pub fn clear_cache(&self) {
        let mut cache = self.process_cache.write().unwrap();
        cache.clear();
        self.gpu_monitor.refresh();
    }

    /// Clear cache for a specific process.
    pub fn clear_process_cache(&self, pid: u32) {
        let mut cache = self.process_cache.write().unwrap();
        cache.remove(&pid);
    }

    /// Maybe refresh system info if cache has expired.
    fn maybe_refresh_system(&self) {
        let should_refresh = {
            let last_refresh = self.last_system_refresh.read().unwrap();
            last_refresh
                .map(|t| t.elapsed() >= self.cache_ttl)
                .unwrap_or(true)
        };

        if should_refresh {
            let mut system = self.system.write().unwrap();
            system.refresh_cpu_all();
            system.refresh_memory();
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::new().with_cpu().with_memory(),
            );

            let mut last_refresh = self.last_system_refresh.write().unwrap();
            *last_refresh = Some(Instant::now());
        }
    }

    /// Check if a process exists.
    pub fn process_exists(&self, pid: u32) -> bool {
        self.maybe_refresh_system();
        let system = self.system.read().unwrap();
        system.process(Pid::from_u32(pid)).is_some()
    }

    /// Get all child PIDs for a process (recursively).
    pub fn get_child_pids(&self, pid: u32) -> Vec<u32> {
        self.maybe_refresh_system();
        let system = self.system.read().unwrap();
        let parent_pid = Pid::from_u32(pid);

        let mut children = Vec::new();
        let mut to_check = vec![parent_pid];

        while let Some(check_pid) = to_check.pop() {
            for (child_pid, process) in system.processes() {
                if process.parent() == Some(check_pid) && *child_pid != parent_pid {
                    children.push(child_pid.as_u32());
                    to_check.push(*child_pid);
                }
            }
        }

        children
    }
}

impl Default for ResourceTracker {
    fn default() -> Self {
        Self::new(Duration::from_secs(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_resources() {
        let tracker = ResourceTracker::default();
        let resources = tracker.get_system_resources().unwrap();

        // Basic sanity checks
        assert!(resources.cpu_usage >= 0.0);
        assert!(resources.ram_total > 0);
        assert!(resources.ram_used <= resources.ram_total);
    }

    #[test]
    fn test_process_resources() {
        let tracker = ResourceTracker::default();

        // Get resources for the current process
        let pid = std::process::id();
        let resources = tracker.get_process_resources(pid, false).unwrap();

        // Should have some RAM usage at least
        assert!(resources.ram_memory >= 0.0);
    }

    #[test]
    fn test_process_exists() {
        let tracker = ResourceTracker::default();

        // Current process should exist
        assert!(tracker.process_exists(std::process::id()));

        // PID 0 typically doesn't exist as a regular process
        // (though it may be the kernel scheduler on some systems)
        // Use a very high PID that's unlikely to exist
        assert!(!tracker.process_exists(999999999));
    }

    #[test]
    fn test_cache_clearing() {
        let tracker = ResourceTracker::default();
        let pid = std::process::id();

        // Get resources to populate cache
        let _ = tracker.get_process_resources(pid, false);

        // Clear cache
        tracker.clear_cache();

        // Should still work after clearing
        let resources = tracker.get_process_resources(pid, false).unwrap();
        assert!(resources.ram_memory >= 0.0);
    }
}
