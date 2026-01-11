#!/usr/bin/env python3
"""
Data models for ComfyUI Version Manager
Defines TypedDict classes for all metadata structures
"""

from datetime import datetime
from typing import Any, Dict, List, Literal, Optional, TypedDict

# ==================== Version Metadata ====================


class VersionInfo(TypedDict, total=False):
    """Information about an installed ComfyUI version"""

    path: str
    installedDate: str  # ISO 8601 format
    pythonVersion: str
    gitCommit: str
    releaseTag: str
    releaseDate: str  # ISO 8601 format
    releaseNotes: str
    downloadUrl: str
    size: int  # bytes
    requirementsHash: str  # sha256 hash
    dependenciesInstalled: bool


class VersionsMetadata(TypedDict):
    """Root metadata structure for versions.json"""

    installed: Dict[str, VersionInfo]  # tag -> version info
    lastSelectedVersion: Optional[str]
    defaultVersion: Optional[str]


# ==================== Custom Node Metadata ====================


class CustomNodeVersionStatus(TypedDict, total=False):
    """Per-version status of a custom node"""

    enabled: bool
    gitCommit: str
    gitTag: Optional[str]
    installDate: str  # ISO 8601
    compatibilityStatus: Literal["compatible", "incompatible", "unknown"]
    incompatibilityReason: Optional[str]
    conflictingPackages: Optional[List[str]]
    requirementsInstalled: bool


class VersionConfig(TypedDict, total=False):
    """Per-version configuration"""

    version: str
    customNodes: Dict[str, CustomNodeVersionStatus]  # node name -> status
    launchArgs: List[str]
    pythonPath: str
    uvPath: str
    requirements: Dict[str, str]  # package -> version
    requirementsHash: str


class CompatibilityCache(TypedDict, total=False):
    """Cached compatibility check result"""

    status: Literal["compatible", "incompatible", "unknown"]
    checkedAt: str  # ISO 8601
    requirementsHash: str
    reason: Optional[str]
    conflictingPackages: Optional[List[str]]
    additionalRequirements: Optional[List[str]]


class CustomNodeInfo(TypedDict, total=False):
    """Global metadata about a custom node"""

    cacheRepo: str  # path to bare git repo
    gitUrl: str
    lastFetched: str  # ISO 8601
    availableTags: List[str]
    latestCommit: str
    hasRequirements: bool
    tags: List[str]  # user-defined tags
    description: str
    compatibilityCache: Dict[str, CompatibilityCache]  # version -> compatibility


CustomNodesMetadata = Dict[str, "CustomNodeInfo"]
"""Root metadata structure for custom_nodes.json."""


# ==================== Model Metadata ====================


class ModelInfo(TypedDict, total=False):
    """Metadata about a model file"""

    path: str
    size: int  # bytes
    sha256: str
    addedDate: str  # ISO 8601
    lastUsed: str  # ISO 8601
    tags: List[str]
    modelType: str  # checkpoint, lora, vae, etc.
    resolution: Optional[str]
    usedByVersions: List[str]
    source: str  # manual, civitai, huggingface, etc.
    baseModel: Optional[str]  # for loras


ModelsMetadata = Dict[str, ModelInfo]
"""Root metadata structure for models.json."""


class ModelHashes(TypedDict, total=False):
    """Hashes for a model's primary file."""

    sha256: str
    blake3: str


class ModelFileInfo(TypedDict, total=False):
    """Metadata about an individual file in a model directory."""

    name: str
    original_name: str
    size: int  # bytes
    sha256: str
    blake3: str


class ModelMetadata(TypedDict, total=False):
    """Canonical metadata stored with each model directory."""

    model_id: str
    family: str
    model_type: str  # llm, diffusion
    subtype: str  # checkpoints, loras, vae, etc.
    official_name: str
    cleaned_name: str
    tags: List[str]
    base_model: str
    preview_image: str
    release_date: str
    download_url: str
    model_card: Dict[str, Any]
    inference_settings: Dict[str, Any]
    compatible_apps: List[str]
    hashes: ModelHashes
    notes: str
    added_date: str  # ISO 8601
    updated_date: str  # ISO 8601
    size_bytes: int
    files: List[ModelFileInfo]
    # Metadata source tracking (Phase 2 - Model Import)
    match_source: str  # 'auto', 'manual', 'hash' - protects user edits
    match_method: str  # 'hash', 'filename_exact', 'filename_fuzzy'
    match_confidence: float  # 0.0-1.0
    # Offline fallback tracking
    pending_online_lookup: bool  # True if HF lookup was skipped due to offline
    lookup_attempts: int  # Number of failed lookup attempts
    last_lookup_attempt: str  # ISO 8601 timestamp of last attempt


class ModelOverrides(TypedDict, total=False):
    """User overrides for model mapping."""

    version_ranges: Dict[str, str]


# ==================== Workflow Metadata ====================


class WorkflowInfo(TypedDict, total=False):
    """Metadata about a workflow file"""

    path: str
    createdDate: str  # ISO 8601
    modifiedDate: str  # ISO 8601
    usedByVersions: List[str]
    tags: List[str]
    description: str
    requiredNodes: List[str]  # custom node names
    requiredModels: List[str]  # model paths


WorkflowsMetadata = Dict[str, WorkflowInfo]
"""Root metadata structure for workflows.json."""


# ==================== GitHub Release Metadata ====================


class GitHubRelease(TypedDict, total=False):
    """GitHub release information"""

    tag_name: str
    name: str
    published_at: str  # ISO 8601
    body: str  # release notes (markdown)
    tarball_url: str
    zipball_url: str
    prerelease: bool
    assets: List[Dict]  # GitHub asset objects
    # Phase 6.2.5c: Size information
    total_size: Optional[int]  # Total download size in bytes
    archive_size: Optional[int]  # ComfyUI archive size in bytes
    dependencies_size: Optional[int]  # Dependencies size in bytes


class GitHubReleasesCache(TypedDict):
    """Cached GitHub releases"""

    lastFetched: str  # ISO 8601
    ttl: int  # seconds
    releases: List[GitHubRelease]


# ==================== Status & Result Types ====================


class DependencyStatus(TypedDict):
    """Status of version dependencies"""

    installed: List[str]
    missing: List[str]
    requirementsFile: Optional[str]


class CompatibilityReport(TypedDict):
    """Compatibility check report for a version"""

    compatible: bool
    issues: List[str]
    warnings: List[str]


class RepairReport(TypedDict):
    """Report from symlink validation/repair"""

    broken: List[str]
    repaired: List[str]
    removed: List[str]


class ScanResult(TypedDict):
    """Result from scanning shared storage"""

    modelsFound: int
    workflowsFound: int
    customNodesFound: int
    totalSize: int  # bytes


class Release(TypedDict):
    """Simplified release info for internal use"""

    tag: str
    name: str
    date: str
    notes: str
    url: str
    prerelease: bool


# ==================== Helper Functions ====================


def get_iso_timestamp() -> str:
    """Get current time as ISO 8601 string"""
    return datetime.utcnow().isoformat() + "Z"


def parse_iso_timestamp(iso_str: str) -> datetime:
    """Parse ISO 8601 string to datetime"""
    # Handle both with and without 'Z' suffix
    if iso_str.endswith("Z"):
        iso_str = iso_str[:-1]
    return datetime.fromisoformat(iso_str)
