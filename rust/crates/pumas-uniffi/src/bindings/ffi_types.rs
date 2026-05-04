use super::{validate_path_string, validate_required_string, FfiResult};
use pumas_library::models::HuggingFaceModel;
use pumas_library::{ModelRecord, SearchResult};

#[derive(uniffi::Record)]
pub struct FfiHashEntry {
    pub key: String,
    pub value: String,
}

#[derive(uniffi::Record)]
pub struct FfiQuantSize {
    pub quant: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSecurityTier {
    Safe,
    Unknown,
    Pickle,
}

impl From<pumas_library::models::SecurityTier> for FfiSecurityTier {
    fn from(t: pumas_library::models::SecurityTier) -> Self {
        match t {
            pumas_library::models::SecurityTier::Safe => FfiSecurityTier::Safe,
            pumas_library::models::SecurityTier::Unknown => FfiSecurityTier::Unknown,
            pumas_library::models::SecurityTier::Pickle => FfiSecurityTier::Pickle,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiFileGroup {
    pub filenames: Vec<String>,
    pub shard_count: u32,
    pub label: String,
}

impl From<pumas_library::models::FileGroup> for FfiFileGroup {
    fn from(g: pumas_library::models::FileGroup) -> Self {
        Self {
            filenames: g.filenames,
            shard_count: g.shard_count,
            label: g.label,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDownloadOption {
    pub quant: String,
    pub size_bytes: Option<u64>,
    pub file_group: Option<FfiFileGroup>,
}

impl From<pumas_library::models::DownloadOption> for FfiDownloadOption {
    fn from(o: pumas_library::models::DownloadOption) -> Self {
        Self {
            quant: o.quant,
            size_bytes: o.size_bytes,
            file_group: o.file_group.map(FfiFileGroup::from),
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiDownloadStatus {
    Queued,
    Downloading,
    Pausing,
    Paused,
    Cancelling,
    Completed,
    Cancelled,
    Error,
}

impl From<pumas_library::models::DownloadStatus> for FfiDownloadStatus {
    fn from(s: pumas_library::models::DownloadStatus) -> Self {
        use pumas_library::models::DownloadStatus;
        match s {
            DownloadStatus::Queued => FfiDownloadStatus::Queued,
            DownloadStatus::Downloading => FfiDownloadStatus::Downloading,
            DownloadStatus::Pausing => FfiDownloadStatus::Pausing,
            DownloadStatus::Paused => FfiDownloadStatus::Paused,
            DownloadStatus::Cancelling => FfiDownloadStatus::Cancelling,
            DownloadStatus::Completed => FfiDownloadStatus::Completed,
            DownloadStatus::Cancelled => FfiDownloadStatus::Cancelled,
            DownloadStatus::Error => FfiDownloadStatus::Error,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiModelImportSpec {
    pub path: String,
    pub family: String,
    pub official_name: String,
    pub repo_id: Option<String>,
    pub model_type: Option<String>,
    pub subtype: Option<String>,
    pub tags: Option<Vec<String>>,
    pub security_acknowledged: Option<bool>,
}

impl FfiModelImportSpec {
    pub(crate) fn into_core(self) -> FfiResult<pumas_library::models::ModelImportSpec> {
        Ok(pumas_library::models::ModelImportSpec {
            path: validate_path_string(self.path, "path")?,
            family: validate_required_string(self.family, "family")?,
            official_name: validate_required_string(self.official_name, "official_name")?,
            repo_id: self.repo_id,
            model_type: self.model_type,
            subtype: self.subtype,
            tags: self.tags,
            security_acknowledged: self.security_acknowledged,
        })
    }
}

#[derive(uniffi::Record)]
pub struct FfiModelImportResult {
    pub path: String,
    pub success: bool,
    pub model_path: Option<String>,
    pub error: Option<String>,
    pub security_tier: Option<FfiSecurityTier>,
}

impl From<pumas_library::models::ModelImportResult> for FfiModelImportResult {
    fn from(r: pumas_library::models::ModelImportResult) -> Self {
        Self {
            path: r.path,
            success: r.success,
            model_path: r.model_path,
            error: r.error,
            security_tier: r.security_tier.map(FfiSecurityTier::from),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiModelDownloadProgress {
    pub download_id: String,
    pub repo_id: Option<String>,
    pub selected_artifact_id: Option<String>,
    pub model_name: Option<String>,
    pub model_type: Option<String>,
    pub status: FfiDownloadStatus,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}

impl From<pumas_library::models::ModelDownloadProgress> for FfiModelDownloadProgress {
    fn from(p: pumas_library::models::ModelDownloadProgress) -> Self {
        Self {
            download_id: p.download_id,
            repo_id: p.repo_id,
            selected_artifact_id: p.selected_artifact_id,
            model_name: p.model_name,
            model_type: p.model_type,
            status: FfiDownloadStatus::from(p.status),
            progress: p.progress,
            downloaded_bytes: p.downloaded_bytes,
            total_bytes: p.total_bytes,
            speed: p.speed,
            eta_seconds: p.eta_seconds,
            error: p.error,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiInterruptedDownload {
    pub model_dir: String,
    pub model_type: Option<String>,
    pub family: String,
    pub inferred_name: String,
    pub part_files: Vec<String>,
    pub completed_files: Vec<String>,
}

impl From<pumas_library::model_library::InterruptedDownload> for FfiInterruptedDownload {
    fn from(d: pumas_library::model_library::InterruptedDownload) -> Self {
        Self {
            model_dir: d.model_dir.to_string_lossy().to_string(),
            model_type: d.model_type,
            family: d.family,
            inferred_name: d.inferred_name,
            part_files: d.part_files,
            completed_files: d.completed_files,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDeleteModelResponse {
    pub success: bool,
    pub error: Option<String>,
}

impl From<pumas_library::models::DeleteModelResponse> for FfiDeleteModelResponse {
    fn from(r: pumas_library::models::DeleteModelResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDiskSpaceResponse {
    pub success: bool,
    pub error: Option<String>,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percent: f32,
}

impl From<pumas_library::models::DiskSpaceResponse> for FfiDiskSpaceResponse {
    fn from(r: pumas_library::models::DiskSpaceResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            total: r.total,
            used: r.used,
            free: r.free,
            percent: r.percent,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiAppResourceUsage {
    pub gpu_memory: Option<u64>,
    pub ram_memory: Option<u64>,
}

impl From<pumas_library::models::AppResourceUsage> for FfiAppResourceUsage {
    fn from(r: pumas_library::models::AppResourceUsage) -> Self {
        Self {
            gpu_memory: r.gpu_memory,
            ram_memory: r.ram_memory,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiAppResources {
    pub comfyui: Option<FfiAppResourceUsage>,
    pub ollama: Option<FfiAppResourceUsage>,
}

impl From<pumas_library::models::AppResources> for FfiAppResources {
    fn from(r: pumas_library::models::AppResources) -> Self {
        Self {
            comfyui: r.comfyui.map(FfiAppResourceUsage::from),
            ollama: r.ollama.map(FfiAppResourceUsage::from),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiStatusResponse {
    pub success: bool,
    pub error: Option<String>,
    pub version: String,
    pub deps_ready: bool,
    pub patched: bool,
    pub menu_shortcut: bool,
    pub desktop_shortcut: bool,
    pub shortcut_version: Option<String>,
    pub message: String,
    pub comfyui_running: bool,
    pub ollama_running: bool,
    pub torch_running: bool,
    pub last_launch_error: Option<String>,
    pub last_launch_log: Option<String>,
    pub app_resources: Option<FfiAppResources>,
}

impl From<pumas_library::models::StatusResponse> for FfiStatusResponse {
    fn from(r: pumas_library::models::StatusResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            version: r.version,
            deps_ready: r.deps_ready,
            patched: r.patched,
            menu_shortcut: r.menu_shortcut,
            desktop_shortcut: r.desktop_shortcut,
            shortcut_version: r.shortcut_version,
            message: r.message,
            comfyui_running: r.comfyui_running,
            ollama_running: r.ollama_running,
            torch_running: r.torch_running,
            last_launch_error: r.last_launch_error,
            last_launch_log: r.last_launch_log,
            app_resources: r.app_resources.map(FfiAppResources::from),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiCpuResources {
    pub usage: f32,
    pub temp: Option<f32>,
}

impl From<pumas_library::models::CpuResources> for FfiCpuResources {
    fn from(r: pumas_library::models::CpuResources) -> Self {
        Self {
            usage: r.usage,
            temp: r.temp,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiGpuResources {
    pub usage: f32,
    pub memory: u64,
    pub memory_total: u64,
    pub temp: Option<f32>,
}

impl From<pumas_library::models::GpuResources> for FfiGpuResources {
    fn from(r: pumas_library::models::GpuResources) -> Self {
        Self {
            usage: r.usage,
            memory: r.memory,
            memory_total: r.memory_total,
            temp: r.temp,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiRamResources {
    pub usage: f32,
    pub total: u64,
}

impl From<pumas_library::models::RamResources> for FfiRamResources {
    fn from(r: pumas_library::models::RamResources) -> Self {
        Self {
            usage: r.usage,
            total: r.total,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDiskResources {
    pub usage: f32,
    pub total: u64,
    pub free: u64,
}

impl From<pumas_library::models::DiskResources> for FfiDiskResources {
    fn from(r: pumas_library::models::DiskResources) -> Self {
        Self {
            usage: r.usage,
            total: r.total,
            free: r.free,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiSystemResources {
    pub cpu: FfiCpuResources,
    pub gpu: FfiGpuResources,
    pub ram: FfiRamResources,
    pub disk: FfiDiskResources,
}

impl From<pumas_library::models::SystemResources> for FfiSystemResources {
    fn from(r: pumas_library::models::SystemResources) -> Self {
        Self {
            cpu: FfiCpuResources::from(r.cpu),
            gpu: FfiGpuResources::from(r.gpu),
            ram: FfiRamResources::from(r.ram),
            disk: FfiDiskResources::from(r.disk),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiSystemResourcesResponse {
    pub success: bool,
    pub error: Option<String>,
    pub resources: FfiSystemResources,
}

impl From<pumas_library::models::SystemResourcesResponse> for FfiSystemResourcesResponse {
    fn from(r: pumas_library::models::SystemResourcesResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            resources: FfiSystemResources::from(r.resources),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiDownloadRequest {
    pub repo_id: String,
    pub family: String,
    pub official_name: String,
    pub model_type: Option<String>,
    pub quant: Option<String>,
    pub filename: Option<String>,
    pub filenames: Option<Vec<String>>,
    pub pipeline_tag: Option<String>,
}

impl FfiDownloadRequest {
    pub(crate) fn into_core(self) -> FfiResult<pumas_library::model_library::DownloadRequest> {
        Ok(pumas_library::model_library::DownloadRequest {
            repo_id: validate_required_string(self.repo_id, "repo_id")?,
            family: validate_required_string(self.family, "family")?,
            official_name: validate_required_string(self.official_name, "official_name")?,
            model_type: self.model_type,
            quant: self.quant,
            filename: self.filename,
            filenames: self.filenames,
            pipeline_tag: self.pipeline_tag,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        })
    }
}

#[derive(uniffi::Record)]
pub struct FfiHfMetadataResult {
    pub repo_id: String,
    pub official_name: Option<String>,
    pub family: Option<String>,
    pub model_type: Option<String>,
    pub subtype: Option<String>,
    pub variant: Option<String>,
    pub precision: Option<String>,
    pub tags: Vec<String>,
    pub base_model: Option<String>,
    pub download_url: Option<String>,
    pub description: Option<String>,
    pub match_confidence: f64,
    pub match_method: String,
    pub requires_confirmation: bool,
    pub hash_mismatch: bool,
    pub matched_filename: Option<String>,
    pub pending_full_verification: bool,
    pub fast_hash: Option<String>,
    pub expected_sha256: Option<String>,
}

impl From<pumas_library::model_library::HfMetadataResult> for FfiHfMetadataResult {
    fn from(r: pumas_library::model_library::HfMetadataResult) -> Self {
        Self {
            repo_id: r.repo_id,
            official_name: r.official_name,
            family: r.family,
            model_type: r.model_type,
            subtype: r.subtype,
            variant: r.variant,
            precision: r.precision,
            tags: r.tags,
            base_model: r.base_model,
            download_url: r.download_url,
            description: r.description,
            match_confidence: r.match_confidence,
            match_method: r.match_method,
            requires_confirmation: r.requires_confirmation,
            hash_mismatch: r.hash_mismatch,
            matched_filename: r.matched_filename,
            pending_full_verification: r.pending_full_verification,
            fast_hash: r.fast_hash,
            expected_sha256: r.expected_sha256,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiLfsFileInfo {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

impl From<pumas_library::model_library::LfsFileInfo> for FfiLfsFileInfo {
    fn from(f: pumas_library::model_library::LfsFileInfo) -> Self {
        Self {
            filename: f.filename,
            size: f.size,
            sha256: f.sha256,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiRepoFileTree {
    pub repo_id: String,
    pub lfs_files: Vec<FfiLfsFileInfo>,
    pub regular_files: Vec<String>,
    pub cached_at: String,
    pub last_modified: Option<String>,
}

impl From<pumas_library::model_library::RepoFileTree> for FfiRepoFileTree {
    fn from(t: pumas_library::model_library::RepoFileTree) -> Self {
        Self {
            repo_id: t.repo_id,
            lfs_files: t.lfs_files.into_iter().map(FfiLfsFileInfo::from).collect(),
            regular_files: t.regular_files,
            cached_at: t.cached_at,
            last_modified: t.last_modified,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiModelRecord {
    pub id: String,
    pub path: String,
    pub cleaned_name: String,
    pub official_name: String,
    pub model_type: String,
    pub tags: Vec<String>,
    pub hashes: Vec<FfiHashEntry>,
    pub metadata_json: String,
    pub updated_at: String,
}

impl From<ModelRecord> for FfiModelRecord {
    fn from(r: ModelRecord) -> Self {
        Self {
            id: r.id,
            path: r.path,
            cleaned_name: r.cleaned_name,
            official_name: r.official_name,
            model_type: r.model_type,
            tags: r.tags,
            hashes: r
                .hashes
                .into_iter()
                .map(|(k, v)| FfiHashEntry { key: k, value: v })
                .collect(),
            metadata_json: r.metadata.to_string(),
            updated_at: r.updated_at,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiSearchResult {
    pub models: Vec<FfiModelRecord>,
    pub total_count: u64,
    pub query_time_ms: f64,
    pub query: String,
}

impl From<SearchResult> for FfiSearchResult {
    fn from(r: SearchResult) -> Self {
        Self {
            models: r.models.into_iter().map(FfiModelRecord::from).collect(),
            total_count: r.total_count as u64,
            query_time_ms: r.query_time_ms,
            query: r.query,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiStringPair {
    pub first: String,
    pub second: String,
}

#[derive(uniffi::Record)]
pub struct FfiReclassifyResult {
    pub total: u64,
    pub reclassified: u64,
    pub changes: Vec<FfiStringPair>,
    pub errors: Vec<FfiStringPair>,
}

impl From<pumas_library::model_library::ReclassifyResult> for FfiReclassifyResult {
    fn from(r: pumas_library::model_library::ReclassifyResult) -> Self {
        Self {
            total: r.total as u64,
            reclassified: r.reclassified as u64,
            changes: r
                .changes
                .into_iter()
                .map(|(f, s)| FfiStringPair {
                    first: f,
                    second: s,
                })
                .collect(),
            errors: r
                .errors
                .into_iter()
                .map(|(f, s)| FfiStringPair {
                    first: f,
                    second: s,
                })
                .collect(),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiHuggingFaceModel {
    pub repo_id: String,
    pub name: String,
    pub developer: String,
    pub kind: String,
    pub formats: Vec<String>,
    pub quants: Vec<String>,
    pub download_options: Vec<FfiDownloadOption>,
    pub url: String,
    pub release_date: Option<String>,
    pub downloads: Option<u64>,
    pub total_size_bytes: Option<u64>,
    pub quant_sizes: Vec<FfiQuantSize>,
    pub compatible_engines: Vec<String>,
}

impl From<HuggingFaceModel> for FfiHuggingFaceModel {
    fn from(m: HuggingFaceModel) -> Self {
        Self {
            repo_id: m.repo_id,
            name: m.name,
            developer: m.developer,
            kind: m.kind,
            formats: m.formats,
            quants: m.quants,
            download_options: m
                .download_options
                .into_iter()
                .map(FfiDownloadOption::from)
                .collect(),
            url: m.url,
            release_date: m.release_date,
            downloads: m.downloads,
            total_size_bytes: m.total_size_bytes,
            quant_sizes: m
                .quant_sizes
                .map(|qs| {
                    qs.into_iter()
                        .map(|(k, v)| FfiQuantSize {
                            quant: k,
                            size_bytes: v,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            compatible_engines: m.compatible_engines,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiParamConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub allowed_values_json: Option<String>,
}

impl From<pumas_library::models::ParamConstraints> for FfiParamConstraints {
    fn from(c: pumas_library::models::ParamConstraints) -> Self {
        Self {
            min: c.min,
            max: c.max,
            allowed_values_json: c
                .allowed_values
                .map(|v| serde_json::to_string(&v).unwrap_or_default()),
        }
    }
}

impl From<FfiParamConstraints> for pumas_library::models::ParamConstraints {
    fn from(c: FfiParamConstraints) -> Self {
        Self {
            min: c.min,
            max: c.max,
            allowed_values: c
                .allowed_values_json
                .and_then(|s| serde_json::from_str(&s).ok()),
        }
    }
}

#[derive(uniffi::Record)]
pub struct FfiInferenceParamSchema {
    pub key: String,
    pub label: String,
    pub param_type: String,
    pub default_json: String,
    pub description: Option<String>,
    pub constraints: Option<FfiParamConstraints>,
}

impl From<pumas_library::models::InferenceParamSchema> for FfiInferenceParamSchema {
    fn from(s: pumas_library::models::InferenceParamSchema) -> Self {
        use pumas_library::models::ParamType;
        Self {
            key: s.key,
            label: s.label,
            param_type: match s.param_type {
                ParamType::Number => "Number".to_string(),
                ParamType::Integer => "Integer".to_string(),
                ParamType::String => "String".to_string(),
                ParamType::Boolean => "Boolean".to_string(),
            },
            default_json: serde_json::to_string(&s.default).unwrap_or_default(),
            description: s.description,
            constraints: s.constraints.map(FfiParamConstraints::from),
        }
    }
}

impl From<FfiInferenceParamSchema> for pumas_library::models::InferenceParamSchema {
    fn from(s: FfiInferenceParamSchema) -> Self {
        use pumas_library::models::ParamType;
        Self {
            key: s.key,
            label: s.label,
            param_type: match s.param_type.as_str() {
                "Integer" => ParamType::Integer,
                "String" => ParamType::String,
                "Boolean" => ParamType::Boolean,
                _ => ParamType::Number,
            },
            default: serde_json::from_str(&s.default_json).unwrap_or(serde_json::Value::Null),
            description: s.description,
            constraints: s
                .constraints
                .map(pumas_library::models::ParamConstraints::from),
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiComputeDevice {
    Cpu,
    Cuda { index: u32 },
    Mps,
    Auto,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSlotState {
    Unloaded,
    Loading,
    Ready,
    Unloading,
    Error,
}

#[derive(uniffi::Record)]
pub struct FfiModelSlot {
    pub slot_id: String,
    pub model_name: String,
    pub model_path: String,
    pub device: FfiComputeDevice,
    pub state: FfiSlotState,
    pub gpu_memory_bytes: Option<u64>,
    pub ram_memory_bytes: Option<u64>,
    pub model_type: Option<String>,
}

#[derive(uniffi::Record)]
pub struct FfiTorchServerConfig {
    pub api_port: u16,
    pub host: String,
    pub max_loaded_models: u32,
    pub lan_access: bool,
}

#[derive(uniffi::Record)]
pub struct FfiDeviceInfo {
    pub device_id: String,
    pub name: String,
    pub memory_total: u64,
    pub memory_available: u64,
    pub is_available: bool,
}

#[derive(uniffi::Record)]
pub struct FfiApiConfig {
    pub launcher_root: String,
    pub auto_create_dirs: bool,
    pub enable_hf: bool,
}
