"""I/O operations for model library."""  # pragma: no cover

from __future__ import annotations  # pragma: no cover

from backend.model_library.io.hashing import (  # pragma: no cover
    StreamHasher,
    compute_dual_hash,
    hash_file_blake3,
    hash_file_sha256,
)
from backend.model_library.io.manager import (  # pragma: no cover
    DriveInfo,
    DriveType,
    IOManager,
    get_drive_info,
    get_drive_type,
)
from backend.model_library.io.validator import (  # pragma: no cover
    ValidationIssue,
    ValidationResult,
    ValidationSeverity,
    is_filesystem_writable,
    is_ntfs_dirty,
    is_path_on_readonly_mount,
    validate_import_source,
    validate_mapping_target,
)

__all__ = [  # pragma: no cover
    # Hashing
    "StreamHasher",
    "compute_dual_hash",
    "hash_file_blake3",
    "hash_file_sha256",
    # Manager
    "DriveInfo",
    "DriveType",
    "IOManager",
    "get_drive_info",
    "get_drive_type",
    # Validation
    "ValidationIssue",
    "ValidationResult",
    "ValidationSeverity",
    "is_filesystem_writable",
    "is_ntfs_dirty",
    "is_path_on_readonly_mount",
    "validate_import_source",
    "validate_mapping_target",
]
