# Under-Covered Code Test Plan

**Date:** 2025-12-30
**Current Coverage:** 27.10%
**Target Coverage:** 80%+
**Status:** Phase 2 In Progress

---

## Executive Summary

This document provides a detailed plan for testing under-covered code modules. The plan focuses on modules with < 60% coverage, prioritizing critical business logic and high-impact code paths.

**Coverage Categories:**
- 🔴 **Critical (0-30%)**: 13 modules - immediate attention needed
- 🟡 **Important (30-60%)**: 8 modules - near-term focus
- 🟢 **Good (60-80%)**: 3 modules - minor gaps
- ✅ **Excellent (80%+)**: 8 modules - maintain coverage

---

## Priority 1: Critical Under-Coverage (0-30%)

### 1. version_manager_components/dependencies.py (8.65%)
**Current:** 370 statements, 338 missing
**Target:** 60% coverage (222 statements)
**Tests Needed:** ~40 tests

#### Missing Coverage Analysis:
- Lines 39-55: Dependency detection from requirements.txt
- Lines 59-67: PyPI version resolution
- Lines 73-103: Constraint file parsing
- Lines 107-131: Dependency installation logic
- Lines 135-248: Pip command execution with retries
- Lines 256-298: Progress tracking during install
- Lines 304-376: Error handling and cleanup
- Lines 380-581: Dependency upgrade/downgrade logic

#### Test Plan:

**Initialization Tests (5 tests):**
```python
def test_init_sets_version_manager()
def test_init_creates_pip_timeout_default()
def test_init_creates_pip_retries_default()
def test_init_with_custom_timeout()
def test_init_with_custom_retries()
```

**Dependency Detection Tests (8 tests):**
```python
def test_detect_dependencies_from_requirements_txt()
def test_detect_dependencies_with_comments()
def test_detect_dependencies_with_empty_lines()
def test_detect_dependencies_with_invalid_format()
def test_detect_dependencies_file_not_found()
def test_detect_dependencies_with_constraints()
def test_detect_dependencies_with_extras()
def test_detect_dependencies_with_urls()
```

**Version Resolution Tests (6 tests):**
```python
def test_resolve_pypi_version_success()
def test_resolve_pypi_version_network_error()
def test_resolve_pypi_version_package_not_found()
def test_resolve_pypi_version_uses_cache()
def test_resolve_pypi_version_respects_constraints()
def test_resolve_pypi_version_with_prerelease()
```

**Installation Tests (12 tests):**
```python
def test_install_dependencies_success()
def test_install_dependencies_with_progress_callback()
def test_install_dependencies_with_constraint_file()
def test_install_dependencies_pip_failure()
def test_install_dependencies_network_timeout()
def test_install_dependencies_retry_logic()
def test_install_dependencies_max_retries_exceeded()
def test_install_dependencies_cancellation()
def test_install_dependencies_with_pip_cache()
def test_install_dependencies_creates_venv_if_missing()
def test_install_dependencies_validates_venv_python()
def test_install_dependencies_logs_output()
```

**Upgrade/Downgrade Tests (5 tests):**
```python
def test_upgrade_dependency_success()
def test_upgrade_dependency_no_newer_version()
def test_downgrade_dependency_success()
def test_upgrade_all_dependencies()
def test_upgrade_with_breaking_changes_detection()
```

**Error Handling Tests (4 tests):**
```python
def test_install_handles_disk_space_error()
def test_install_handles_permission_error()
def test_install_cleans_up_on_failure()
def test_install_reports_specific_package_failure()
```

---

### 2. version_manager_components/installer.py (10.88%)
**Current:** 294 statements, 262 missing
**Target:** 60% coverage (176 statements)
**Tests Needed:** ~35 tests

#### Missing Coverage Analysis:
- Lines 39-55: Installation initialization
- Lines 59-66: Version directory setup
- Lines 75-157: GitHub archive download
- Lines 172-241: Archive extraction
- Lines 246-299: Virtual environment creation
- Lines 304-374: Python interpreter validation
- Lines 379-453: Metadata persistence
- Lines 466-503: Installation cleanup

#### Test Plan:

**Installation Initialization (6 tests):**
```python
def test_start_installation_creates_version_dir()
def test_start_installation_validates_tag()
def test_start_installation_checks_existing_installation()
def test_start_installation_creates_progress_tracker()
def test_start_installation_sets_installing_flag()
def test_start_installation_with_custom_log_path()
```

**Download Tests (8 tests):**
```python
def test_download_archive_from_github()
def test_download_archive_with_progress_callback()
def test_download_archive_network_error_retry()
def test_download_archive_checksum_validation()
def test_download_archive_cancellation()
def test_download_archive_disk_space_check()
def test_download_archive_uses_cache()
def test_download_archive_timeout_handling()
```

**Extraction Tests (6 tests):**
```python
def test_extract_archive_to_version_dir()
def test_extract_archive_with_progress()
def test_extract_archive_invalid_format()
def test_extract_archive_corrupted_file()
def test_extract_archive_permission_error()
def test_extract_archive_creates_correct_structure()
```

**Virtual Environment Tests (7 tests):**
```python
def test_create_venv_with_system_python()
def test_create_venv_validates_python_version()
def test_create_venv_creation_failure()
def test_create_venv_with_specific_python()
def test_create_venv_includes_pip()
def test_create_venv_inherits_packages_option()
def test_create_venv_symlink_handling()
```

**Metadata Tests (4 tests):**
```python
def test_save_installation_metadata()
def test_load_installation_metadata()
def test_update_installation_metadata()
def test_metadata_atomic_write()
```

**Cleanup Tests (4 tests):**
```python
def test_cleanup_on_success()
def test_cleanup_on_failure_removes_partial()
def test_cleanup_preserves_logs()
def test_cleanup_handles_locked_files()
```

---

### 3. resource_manager.py (9.12%)
**Current:** 318 statements, 289 missing
**Target:** 60% coverage (191 statements)
**Tests Needed:** ~40 tests

#### Missing Coverage Analysis:
- Lines 38-50: Resource manager initialization
- Lines 59-73: Resource tracking setup
- Lines 86-117: Version resource allocation
- Lines 126-172: Resource deallocation
- Lines 187-211: Resource limits enforcement
- Lines 223-263: Memory tracking
- Lines 278-370: Disk space management
- Lines 379-421: Resource cleanup
- Lines 431-461: Resource reporting
- Lines 473-532: Resource prediction
- Lines 544-625: Multi-version resource coordination
- Lines 638-693: Resource migration
- Lines 706-740: Resource validation

#### Test Plan:

**Initialization Tests (5 tests):**
```python
def test_init_sets_launcher_root()
def test_init_creates_resource_tracker()
def test_init_loads_resource_config()
def test_init_validates_system_resources()
def test_init_sets_default_limits()
```

**Resource Allocation Tests (8 tests):**
```python
def test_allocate_resources_for_version()
def test_allocate_resources_with_insufficient_memory()
def test_allocate_resources_with_insufficient_disk()
def test_allocate_resources_respects_limits()
def test_allocate_resources_tracks_allocation()
def test_allocate_resources_concurrent_versions()
def test_allocate_resources_priority_handling()
def test_allocate_resources_cleanup_on_failure()
```

**Resource Deallocation Tests (6 tests):**
```python
def test_deallocate_resources_for_stopped_version()
def test_deallocate_resources_updates_tracker()
def test_deallocate_resources_cleans_temp_files()
def test_deallocate_resources_releases_locks()
def test_deallocate_resources_handles_missing_version()
def test_deallocate_resources_partial_cleanup()
```

**Memory Tracking Tests (6 tests):**
```python
def test_track_memory_usage()
def test_track_memory_per_version()
def test_track_memory_exceeds_limit_warning()
def test_track_memory_includes_subprocess()
def test_track_memory_update_interval()
def test_track_memory_cleanup_stale_data()
```

**Disk Space Tests (7 tests):**
```python
def test_check_disk_space_available()
def test_check_disk_space_insufficient()
def test_estimate_version_disk_usage()
def test_cleanup_old_versions_by_size()
def test_cleanup_cache_to_free_space()
def test_disk_space_monitoring()
def test_disk_space_warning_threshold()
```

**Resource Prediction Tests (4 tests):**
```python
def test_predict_installation_resources()
def test_predict_runtime_resources()
def test_predict_upgrade_resources()
def test_predict_resources_with_dependencies()
```

**Resource Reporting Tests (4 tests):**
```python
def test_get_resource_summary()
def test_get_version_resource_usage()
def test_get_system_resource_availability()
def test_generate_resource_report()
```

---

### 4. shortcut_manager.py (12.32%)
**Current:** 341 statements, 299 missing
**Target:** 60% coverage (205 statements)
**Tests Needed:** ~30 tests

#### Missing Coverage Analysis:
- Lines 25-28: Platform detection
- Lines 54-64: Icon path resolution
- Lines 68-89: Icon generation from SVG
- Lines 98-132: Desktop file creation (Linux)
- Lines 137-160: Shortcut creation (Windows)
- Lines 164-176: Shortcut creation (macOS)
- Lines 180-214: Icon caching
- Lines 223-256: Shortcut validation
- Lines 260-354: Shortcut updates
- Lines 360-465: Multi-version shortcut management
- Lines 469-590: Desktop integration
- Lines 596-707: Shortcut removal and cleanup

#### Test Plan:

**Platform Detection Tests (3 tests):**
```python
def test_platform_detection_linux()
def test_platform_detection_windows()
def test_platform_detection_macos()
```

**Icon Tests (6 tests):**
```python
def test_generate_icon_from_svg()
def test_generate_icon_with_custom_size()
def test_generate_icon_caches_result()
def test_generate_icon_fallback_to_default()
def test_generate_icon_multiple_sizes()
def test_icon_cache_expiration()
```

**Desktop File Tests (Linux) (7 tests):**
```python
def test_create_desktop_file()
def test_create_desktop_file_with_version()
def test_desktop_file_content_validation()
def test_desktop_file_permissions()
def test_desktop_file_in_applications_dir()
def test_desktop_file_update_existing()
def test_desktop_file_categories()
```

**Windows Shortcut Tests (5 tests):**
```python
@pytest.mark.skipif(sys.platform != "win32", reason="Windows only")
def test_create_windows_shortcut()
def test_windows_shortcut_icon()
def test_windows_shortcut_working_dir()
def test_windows_shortcut_arguments()
def test_windows_shortcut_update()
```

**macOS Shortcut Tests (4 tests):**
```python
@pytest.mark.skipif(sys.platform != "darwin", reason="macOS only")
def test_create_macos_app_bundle()
def test_macos_app_info_plist()
def test_macos_app_icon()
def test_macos_app_executable()
```

**Shortcut Management Tests (5 tests):**
```python
def test_create_shortcut_for_version()
def test_remove_shortcut_for_version()
def test_update_shortcut_for_version()
def test_list_existing_shortcuts()
def test_validate_shortcut_integrity()
```

---

### 5. api/core.py (20.53%)
**Current:** 453 statements, 360 missing
**Target:** 60% coverage (272 statements)
**Tests Needed:** ~50 tests

#### Missing Coverage Analysis:
- Lines 46-96: ComfyUISetupAPI initialization
- Lines 103-143: Version installation endpoint
- Lines 147-181: Version switching endpoint
- Lines 186-215: Version uninstallation endpoint
- Lines 233-287: Launch endpoint with process management
- Lines 291-407: Stop/status endpoints
- Lines 411-465: Model management endpoints
- Lines 479-559: Custom nodes endpoints
- Lines 563-621: Configuration endpoints
- Lines 625-698: Cache management endpoints
- Lines 706-782: Resource management endpoints
- Lines 786-800: Cleanup and utility endpoints

#### Test Plan:

**API Initialization (4 tests):**
```python
def test_api_init_creates_version_manager()
def test_api_init_creates_metadata_manager()
def test_api_init_validates_launcher_root()
def test_api_init_sets_default_config()
```

**Version Installation Endpoint (10 tests):**
```python
def test_install_version_success()
def test_install_version_already_installed()
def test_install_version_invalid_tag()
def test_install_version_network_error()
def test_install_version_disk_space_error()
def test_install_version_with_progress_updates()
def test_install_version_cancellation()
def test_install_version_concurrent_install_blocked()
def test_install_version_validates_dependencies()
def test_install_version_cleanup_on_failure()
```

**Version Switching Endpoint (6 tests):**
```python
def test_switch_version_success()
def test_switch_version_not_installed()
def test_switch_version_while_running()
def test_switch_version_updates_metadata()
def test_switch_version_preserves_config()
def test_switch_version_migrates_resources()
```

**Launch Endpoint (8 tests):**
```python
def test_launch_version_success()
def test_launch_version_not_installed()
def test_launch_version_already_running()
def test_launch_version_with_custom_args()
def test_launch_version_port_conflict()
def test_launch_version_process_tracking()
def test_launch_version_log_file_creation()
def test_launch_version_resource_limits()
```

**Stop/Status Endpoints (6 tests):**
```python
def test_stop_version_graceful()
def test_stop_version_force_kill()
def test_stop_version_not_running()
def test_get_status_running()
def test_get_status_stopped()
def test_get_status_installing()
```

**Model Management Endpoints (8 tests):**
```python
def test_list_models()
def test_add_model_symlink()
def test_remove_model_symlink()
def test_get_model_info()
def test_model_storage_management()
def test_model_sharing_between_versions()
def test_model_validation()
def test_model_download_tracking()
```

**Configuration Endpoints (8 tests):**
```python
def test_get_version_config()
def test_update_version_config()
def test_get_global_config()
def test_update_global_config()
def test_reset_config_to_defaults()
def test_config_validation()
def test_config_migration()
def test_config_backup_restore()
```

---

## Priority 2: Important Under-Coverage (30-60%)

### 6. github_integration.py (37.24%)
**Current:** 333 statements, 209 missing
**Target:** 70% coverage (233 statements)
**Tests Needed:** ~15 tests (DownloadManager focus)

#### Missing Coverage - DownloadManager:
- Lines 513-598: Download initialization and setup
- Lines 492-493: Cancellation handling
- Lines 622-633: Progress callbacks
- Lines 646-651: Checksum validation
- Lines 662-670: Download cleanup

#### Test Plan:

**DownloadManager Initialization (3 tests):**
```python
def test_download_manager_init()
def test_download_manager_creates_temp_dir()
def test_download_manager_validates_url()
```

**Download Tests (6 tests):**
```python
def test_download_file_success()
def test_download_file_with_progress()
def test_download_file_network_error()
def test_download_file_cancellation()
def test_download_file_resume_partial()
def test_download_file_checksum_validation()
```

**Cleanup Tests (3 tests):**
```python
def test_cleanup_temp_files()
def test_cleanup_on_error()
def test_cleanup_preserves_complete_downloads()
```

**Integration Tests (3 tests):**
```python
def test_download_github_release_archive()
def test_download_with_rate_limiting()
def test_download_concurrent_files()
```

---

### 7. installation_progress_tracker.py (25.57%)
**Current:** 176 statements, 131 missing
**Target:** 70% coverage (123 statements)
**Tests Needed:** ~30 tests

#### Missing Coverage Analysis:
- Lines 103-122: start_installation
- Lines 135-146: update_stage
- Lines 162-188: update_download_progress
- Lines 206-221: update_dependency_progress
- Lines 232-244: add_completed_item
- Lines 253-267: set_dependency_weights
- Lines 276-291: complete_package
- Lines 304-313: _extract_package_name
- Lines 322-341: set_error/set_pid
- Lines 350-367: complete_installation
- Lines 376-410: _calculate_overall_progress
- Lines 414-435: _save_state/_load_state

#### Test Plan:

**Initialization Tests (4 tests):**
```python
def test_init_creates_cache_dir()
def test_init_creates_state_file_path()
def test_init_initializes_locks()
def test_init_clears_stale_state()
```

**Installation Lifecycle Tests (6 tests):**
```python
def test_start_installation()
def test_start_installation_with_size_and_count()
def test_start_installation_creates_state()
def test_complete_installation_success()
def test_complete_installation_failure()
def test_clear_state()
```

**Progress Tracking Tests (8 tests):**
```python
def test_update_stage()
def test_update_download_progress()
def test_update_download_progress_calculates_eta()
def test_update_dependency_progress()
def test_update_dependency_progress_calculates_percentage()
def test_add_completed_item()
def test_get_current_state()
def test_get_current_state_returns_copy()
```

**Package Weight Tests (6 tests):**
```python
def test_set_dependency_weights()
def test_set_dependency_weights_with_torch()
def test_complete_package_updates_weight()
def test_extract_package_name_from_spec()
def test_extract_package_name_with_extras()
def test_package_weight_defaults()
```

**Overall Progress Calculation (3 tests):**
```python
def test_calculate_overall_progress_download_stage()
def test_calculate_overall_progress_dependencies_stage()
def test_calculate_overall_progress_all_stages()
```

**State Persistence (3 tests):**
```python
def test_save_state_atomic_write()
def test_load_state_from_disk()
def test_state_survives_crash()
```

---

### 8. metadata_manager.py (28.85%)
**Current:** 104 statements, 74 missing
**Target:** 80% coverage (83 statements)
**Tests Needed:** ~25 tests

#### Missing Coverage Analysis:
- Lines 37-55: Initialization and directory creation
- Lines 74-91: _read_json with error handling
- Lines 108-120: _write_json with atomic writes
- Lines 131-148: load_versions/save_versions
- Lines 162-179: version config operations
- Lines 191-195: delete_version_config
- Lines 206-264: models/custom_nodes/workflows metadata
- Lines 275-289: GitHub cache operations
- Lines 300-340: utility methods

#### Test Plan:

**Initialization Tests (5 tests):**
```python
def test_init_creates_directories()
def test_init_sets_file_paths()
def test_init_creates_write_lock()
def test_init_with_existing_directories()
def test_init_validates_launcher_data_dir()
```

**JSON Operations Tests (6 tests):**
```python
def test_read_json_success()
def test_read_json_file_not_found_returns_default()
def test_read_json_invalid_json_raises_error()
def test_read_json_io_error_with_no_default()
def test_write_json_success()
def test_write_json_serialization_error()
```

**Versions Metadata Tests (4 tests):**
```python
def test_load_versions_empty_default()
def test_load_versions_with_existing_data()
def test_save_versions_atomic_write()
def test_save_versions_creates_backup()
```

**Version Config Tests (4 tests):**
```python
def test_load_version_config_exists()
def test_load_version_config_not_found()
def test_save_version_config()
def test_delete_version_config()
```

**Resource Metadata Tests (3 tests):**
```python
def test_load_save_models_metadata()
def test_load_save_custom_nodes_metadata()
def test_load_save_workflows_metadata()
```

**GitHub Cache Tests (3 tests):**
```python
def test_load_github_cache()
def test_save_github_cache()
def test_github_cache_not_found()
```

---

## Priority 3: Minor Gaps (60-80%)

### 9. patch_manager.py (80.67%)
**Current:** 119 statements, 23 missing
**Target:** 90% coverage (107 statements)
**Tests Needed:** ~8 tests

#### Missing Lines:
- Lines 69-73: Apply patch error handling
- Lines 84-90: Patch validation
- Lines 98-103: Patch conflict resolution
- Lines 119: Edge case in patch detection
- Lines 128-130: Patch reversal
- Lines 153-155: Patch status tracking
- Lines 217: Cleanup edge case

---

## Summary Statistics

### Tests to Create by Priority:

| Priority | Module | Tests | Coverage Gain | Effort |
|----------|--------|-------|---------------|--------|
| P1 | dependencies.py | 40 | 51.35% | High |
| P1 | installer.py | 35 | 49.12% | High |
| P1 | resource_manager.py | 40 | 50.88% | High |
| P1 | shortcut_manager.py | 30 | 47.68% | Medium |
| P1 | core.py | 50 | 39.47% | High |
| P2 | github_integration (DownloadManager) | 15 | 32.76% | Medium |
| P2 | installation_progress_tracker.py | 30 | 44.43% | Medium |
| P2 | metadata_manager.py | 25 | 51.15% | Low |
| P3 | patch_manager.py | 8 | 9.33% | Low |

**Total New Tests:** ~273 tests
**Total Coverage Gain:** ~52.9%
**Estimated New Coverage:** 80%+

---

## Implementation Timeline

### Week 1-2: metadata_manager.py + installation_progress_tracker.py
- **Tests:** 55
- **Coverage gain:** ~10%
- **Focus:** Foundation modules with clear boundaries

### Week 3-4: github_integration.py (DownloadManager) + shortcut_manager.py
- **Tests:** 45
- **Coverage gain:** ~8%
- **Focus:** User-facing features

### Week 5-6: dependencies.py + installer.py
- **Tests:** 75
- **Coverage gain:** ~15%
- **Focus:** Core installation logic

### Week 7-8: resource_manager.py + core.py
- **Tests:** 90
- **Coverage gain:** ~18%
- **Focus:** API and resource management

### Week 9: Remaining gaps + patch_manager.py
- **Tests:** 8
- **Coverage gain:** ~2%
- **Focus:** Polish and edge cases

---

## Next Actions

1. ✅ **Create test_metadata_manager.py** (25 tests) - CURRENT
2. Create test_installation_progress_tracker.py (30 tests)
3. Expand test_github_integration.py with DownloadManager tests (15 tests)
4. Create test_version_manager_components/test_dependencies.py (40 tests)
5. Create test_version_manager_components/test_installer.py (35 tests)

---

**Last Updated:** 2025-12-30
**Target Completion:** 9 weeks (~180 hours)
