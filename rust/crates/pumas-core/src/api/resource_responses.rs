//! Shared API response conversion helpers for resource snapshots.

use crate::{models, system};

pub(super) fn system_resources_response_from_snapshot(
    snapshot: system::SystemResourceSnapshot,
) -> models::SystemResourcesResponse {
    let ram_usage = if snapshot.ram_total > 0 {
        (snapshot.ram_used as f32 / snapshot.ram_total as f32) * 100.0
    } else {
        0.0
    };
    let disk_usage = if snapshot.disk_total > 0 {
        (snapshot.disk_used as f32 / snapshot.disk_total as f32) * 100.0
    } else {
        0.0
    };

    models::SystemResourcesResponse {
        success: true,
        error: None,
        resources: models::SystemResources {
            cpu: models::CpuResources {
                usage: snapshot.cpu_usage,
                temp: snapshot.cpu_temp,
            },
            gpu: models::GpuResources {
                usage: snapshot.gpu_usage,
                memory: snapshot.gpu_memory_used,
                memory_total: snapshot.gpu_memory_total,
                temp: snapshot.gpu_temp,
            },
            ram: models::RamResources {
                usage: ram_usage,
                total: snapshot.ram_total,
            },
            disk: models::DiskResources {
                usage: disk_usage,
                total: snapshot.disk_total,
                free: snapshot.disk_free,
            },
        },
    }
}
