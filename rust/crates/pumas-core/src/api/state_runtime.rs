//! Runtime status helpers used by primary-state IPC dispatch.

use super::state::{PrimaryState, launcher_root_from_primary};
use crate::error::PumasError;
use crate::{models, network};

pub(super) fn disk_space_response(
    primary: &PrimaryState,
) -> std::result::Result<models::DiskSpaceResponse, PumasError> {
    use sysinfo::Disks;

    let launcher_root = launcher_root_from_primary(primary);
    let launcher_root_str = launcher_root.to_string_lossy();
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.list() {
        let mount_point = disk.mount_point().to_string_lossy();
        if launcher_root_str.starts_with(mount_point.as_ref()) {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            return Ok(models::DiskSpaceResponse {
                success: true,
                error: None,
                total,
                used,
                free,
                percent,
            });
        }
    }

    if let Some(disk) = disks.list().first() {
        let total = disk.total_space();
        let free = disk.available_space();
        let used = total.saturating_sub(free);
        let percent = if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        return Ok(models::DiskSpaceResponse {
            success: true,
            error: None,
            total,
            used,
            free,
            percent,
        });
    }

    Err(PumasError::Other(
        "Could not determine disk space".to_string(),
    ))
}

pub(super) async fn status_response(
    primary: &PrimaryState,
) -> std::result::Result<models::StatusResponse, PumasError> {
    let mgr_lock = primary.process_manager.read().await;
    let comfyui_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_running());
    let ollama_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_ollama_running());
    let torch_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_torch_running());
    let last_launch_error = mgr_lock.as_ref().and_then(|mgr| mgr.last_launch_error());
    let last_launch_log = mgr_lock.as_ref().and_then(|mgr| {
        mgr.last_launch_log()
            .map(|p| p.to_string_lossy().to_string())
    });

    let app_resources = if let Some(ref mgr) = *mgr_lock {
        let comfyui_resources = if comfyui_running {
            mgr.aggregate_app_resources()
                .map(|r| models::AppResourceUsage {
                    gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                })
        } else {
            None
        };

        let ollama_resources = if ollama_running {
            mgr.aggregate_ollama_resources()
                .map(|r| models::AppResourceUsage {
                    gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                })
        } else {
            None
        };

        if comfyui_resources.is_some() || ollama_resources.is_some() {
            Some(models::AppResources {
                comfyui: comfyui_resources,
                ollama: ollama_resources,
            })
        } else {
            None
        }
    } else {
        None
    };
    drop(mgr_lock);

    Ok(models::StatusResponse {
        success: true,
        error: None,
        version: env!("CARGO_PKG_VERSION").to_string(),
        deps_ready: true,
        patched: false,
        menu_shortcut: false,
        desktop_shortcut: false,
        shortcut_version: None,
        message: if comfyui_running {
            "ComfyUI running".to_string()
        } else if ollama_running {
            "Ollama running".to_string()
        } else if torch_running {
            "Torch running".to_string()
        } else {
            "Ready".to_string()
        },
        comfyui_running,
        ollama_running,
        torch_running,
        last_launch_error,
        last_launch_log,
        app_resources,
    })
}

pub(super) async fn system_resources_response(
    primary: &PrimaryState,
) -> std::result::Result<models::SystemResourcesResponse, PumasError> {
    use sysinfo::{Disks, System};

    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let ram_usage = if total_memory > 0 {
        (used_memory as f32 / total_memory as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_free) = if let Some(disk) = disks.list().first() {
        (disk.total_space(), disk.available_space())
    } else {
        (0, 0)
    };
    let disk_usage = if disk_total > 0 {
        ((disk_total - disk_free) as f32 / disk_total as f32) * 100.0
    } else {
        0.0
    };

    let gpu = if let Some(ref mgr) = *primary.process_manager.read().await {
        let tracker = mgr.resource_tracker();
        match tracker.get_system_resources() {
            Ok(snapshot) => models::GpuResources {
                usage: snapshot.gpu_usage,
                memory: snapshot.gpu_memory_used,
                memory_total: snapshot.gpu_memory_total,
                temp: snapshot.gpu_temp,
            },
            Err(_) => models::GpuResources {
                usage: 0.0,
                memory: 0,
                memory_total: 0,
                temp: None,
            },
        }
    } else {
        models::GpuResources {
            usage: 0.0,
            memory: 0,
            memory_total: 0,
            temp: None,
        }
    };

    Ok(models::SystemResourcesResponse {
        success: true,
        error: None,
        resources: models::SystemResources {
            cpu: models::CpuResources {
                usage: cpu_usage,
                temp: None,
            },
            gpu,
            ram: models::RamResources {
                usage: ram_usage,
                total: total_memory,
            },
            disk: models::DiskResources {
                usage: disk_usage,
                total: disk_total,
                free: disk_free,
            },
        },
    })
}

pub(super) async fn network_status_response(
    primary: &PrimaryState,
) -> models::NetworkStatusResponse {
    let status = primary.network_manager.status().await;

    let mut total_successful_requests: u64 = 0;
    let mut total_failed_requests: u64 = 0;
    let mut circuit_states = std::collections::HashMap::new();
    let mut any_open_circuit = false;

    for breaker in &status.circuit_breakers {
        total_successful_requests += breaker.total_successes;
        total_failed_requests += breaker.total_failures;
        let state = breaker.state.to_string();
        if state == "OPEN" {
            any_open_circuit = true;
        }
        circuit_states.insert(breaker.domain.clone(), state);
    }

    let total_requests = total_successful_requests + total_failed_requests;
    let success_rate = if total_requests > 0 {
        total_successful_requests as f64 / total_requests as f64
    } else {
        1.0
    };

    models::NetworkStatusResponse {
        success: true,
        error: None,
        total_requests,
        successful_requests: total_successful_requests,
        failed_requests: total_failed_requests,
        circuit_breaker_rejections: 0,
        retries: 0,
        success_rate,
        circuit_states,
        is_offline: status.connectivity == network::ConnectivityState::Offline || any_open_circuit,
    }
}
