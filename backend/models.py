#!/usr/bin/env python3
"""
Data models for ComfyUI Version Manager
Defines TypedDict classes for all metadata structures
"""

from typing import TypedDict, Dict, List, Optional, Literal
from datetime import datetime


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


class CustomNodesMetadata(TypedDict):
    """Root metadata structure for custom_nodes.json"""
    # node name -> node info (note: this is a dict, not TypedDict)


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


class ModelsMetadata(TypedDict):
    """Root metadata structure for models.json"""
    # relative path -> model info (note: this is a dict, not TypedDict)


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


class WorkflowsMetadata(TypedDict):
    """Root metadata structure for workflows.json"""
    # workflow filename -> workflow info (note: this is a dict, not TypedDict)


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
    status: Literal["complete", "incomplete", "unknown"]
    missing: List[str]
    outdated: List[Dict[str, str]]  # package, required, installed
    satisfied: List[str]


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
    if iso_str.endswith('Z'):
        iso_str = iso_str[:-1]
    return datetime.fromisoformat(iso_str)
