# Comprehensive Unit Test Plan - 80%+ Coverage Target

**Objective:** Achieve at least 80% code coverage across all backend Python modules with high-quality, maintainable tests.

**Current Status:** 112 tests passing (10 existing + 102 new from Phase 1)

**Last Updated:** 2025-12-30

---

## Executive Summary

This plan details comprehensive unit test coverage for the ComfyUI Launcher backend, organized by priority and complexity. The plan builds on the existing Phase 1 implementation (resource tracking) and extends to cover all critical backend modules.

### Overall Progress
- **Current Coverage:** ~30% (estimated across all backend modules)
- **Target Coverage:** 80%+ overall
- **Tests Completed:** 112 tests
- **Tests Remaining:** ~350-400 tests (estimated)
- **Timeline:** 6-8 weeks for full implementation

---

## Priority Matrix

| Priority | Module | Current Coverage | Target | Tests Needed | Effort |
|----------|--------|------------------|--------|--------------|--------|
| **P0** | process_resource_tracker.py | 95% | 95% | ✅ Complete | - |
| **P0** | process_manager.py | 75% | 90% | +20 tests | 2 days |
| **P0** | system_utils.py | 65% | 85% | +25 tests | 2 days |
| **P1** | dependency_manager.py | 85% | 90% | +5 tests | 1 day |
| **P1** | patch_manager.py | 85% | 90% | +5 tests | 1 day |
| **P1** | github_integration.py | 0% | 80% | +40 tests | 3 days |
| **P1** | version_manager.py | 0% | 75% | +35 tests | 3 days |
| **P2** | shortcut_manager.py | 0% | 75% | +30 tests | 3 days |
| **P2** | metadata_manager.py | 0% | 80% | +25 tests | 2 days |
| **P2** | core.py (ComfyUISetupAPI) | 15% | 70% | +50 tests | 4 days |
| **P2** | main.py (JavaScriptAPI) | 25% | 75% | +45 tests | 3 days |
| **P3** | size_calculator.py | 0% | 75% | +20 tests | 2 days |
| **P3** | version_info.py | 0% | 80% | +15 tests | 1 day |
| **P3** | resource_manager.py | 0% | 70% | +25 tests | 2 days |
| **P3** | release_size_calculator.py | 0% | 65% | +20 tests | 2 days |
| **P4** | validators.py | 0% | 90% | +15 tests | 1 day |
| **P4** | utils.py | 0% | 70% | +20 tests | 2 days |
| **P4** | file_utils.py | 0% | 80% | +12 tests | 1 day |
| **P4** | models.py | 0% | 70% | +18 tests | 2 days |

---

## Phase 1: Critical Path (COMPLETED ✅)

### 1.1 process_resource_tracker.py ✅
**Status:** COMPLETE - 95% coverage (33 tests)
**Test File:** `backend/tests/test_process_resource_tracker.py`

**Coverage:**
- ✅ CPU tracking (single, children, errors)
- ✅ RAM tracking (single, children, errors)
- ✅ GPU tracking (nvidia-smi)
- ✅ Caching (TTL, hits, misses)
- ✅ Error handling (NoSuchProcess, AccessDenied, ZombieProcess)

### 1.2 dependency_manager.py ✅
**Status:** COMPLETE - 85% coverage (20 tests)
**Test File:** `backend/tests/test_dependency_manager.py`

**Coverage:**
- ✅ Initialization
- ✅ Dependency checking (setproctitle, git, brave)
- ✅ Missing dependency detection
- ✅ Installation workflows (Python packages, system packages)

**Remaining Gaps (5 tests needed for 90%):**
- ❌ `_install_package()` - sudo password prompts
- ❌ `_install_package()` - apt lock errors
- ❌ `check_dependencies()` - partial installation failures
- ❌ Multiple sequential installation attempts
- ❌ Package installation timeout scenarios

### 1.3 patch_manager.py ✅
**Status:** COMPLETE - 85% coverage (27 tests)
**Test File:** `backend/tests/test_patch_manager.py`

**Coverage:**
- ✅ Patch detection (patched, unpatched, version-specific)
- ✅ Patch application (success, already patched, upgrades)
- ✅ Patch reversion (backup, git, GitHub)
- ✅ Version-specific targeting

**Remaining Gaps (5 tests needed for 90%):**
- ❌ Concurrent patch operations (file locking)
- ❌ Corrupted backup file handling
- ❌ Partial git restore failures
- ❌ Permission errors during patch write
- ❌ Patch version migration scenarios

### 1.4 process_manager.py 🔶
**Status:** IN PROGRESS - 75% coverage (10 existing + 30 new = 40 tests)
**Test File:** `backend/tests/test_process_manager.py` + `test_process_manager_extended.py`

**Existing Coverage:**
- ✅ Resource enrichment (7 tests)
- ✅ Error handling (3 tests)
- ✅ Process detection (7 tests from extended)
- ✅ is_comfyui_running (3 tests from extended)
- ✅ stop_comfyui (5 tests from extended)
- ✅ launch_comfyui (3 tests from extended)

**Remaining Gaps (20 tests needed for 90%):**
- ❌ `_get_known_version_paths()` - multiple versions
- ❌ `_get_known_version_paths()` - missing venv scenarios
- ❌ `_detect_comfyui_processes()` - concurrent version runs
- ❌ `_detect_comfyui_processes()` - orphaned PID files cleanup
- ❌ `get_processes_with_resources()` - aggregation edge cases
- ❌ Process filtering by version tag
- ❌ Resource enrichment with partial data
- ❌ Stop operations with child processes
- ❌ Launch with missing dependencies
- ❌ Launch with port conflicts
- ❌ Process name validation edge cases
- ❌ PID file race conditions
- ❌ Multiple simultaneous launches
- ❌ Graceful vs force kill scenarios
- ❌ Browser window cleanup edge cases
- ❌ Process tree traversal with deep nesting
- ❌ Resource tracking during process lifecycle
- ❌ Integration with version manager state
- ❌ Error recovery mechanisms
- ❌ Signal handling edge cases

### 1.5 system_utils.py 🔶
**Status:** IN PROGRESS - 65% coverage (10 existing + 25 new = 35 tests)
**Test File:** `backend/tests/test_system_utils.py` + `test_system_utils_extended.py`

**Existing Coverage:**
- ✅ get_status() - no processes (10 tests)
- ✅ Disk space operations (2 tests from extended)
- ✅ Toggle patch (2 tests from extended)
- ✅ Toggle menu/desktop (5 tests from extended)
- ✅ Open path/URL (5 tests from extended)
- ✅ System resources (4 tests from extended)

**Remaining Gaps (25 tests needed for 85%):**
- ❌ `get_status()` - mixed process states
- ❌ `get_status()` - resource aggregation edge cases
- ❌ `get_status()` - process version detection
- ❌ `get_disk_space()` - various filesystem types
- ❌ `get_disk_space()` - permission errors
- ❌ `get_disk_space()` - symlink resolution
- ❌ `toggle_patch()` - concurrent operations
- ❌ `toggle_menu()` - multiple versions
- ❌ `toggle_desktop()` - permission failures
- ❌ `open_path()` - special characters in paths
- ❌ `open_path()` - network paths
- ❌ `open_url()` - malformed URLs
- ❌ `open_url()` - browser selection logic
- ❌ `open_active_install()` - various states
- ❌ `get_system_resources()` - GPU detection failures
- ❌ `get_system_resources()` - multiple GPUs
- ❌ `get_system_resources()` - CPU count variations
- ❌ `get_system_resources()` - memory edge cases
- ❌ Integration with process manager
- ❌ Error propagation testing
- ❌ Rate limiting verification
- ❌ Caching behavior validation
- ❌ Concurrent request handling
- ❌ Resource cleanup verification
- ❌ State consistency checks

---

## Phase 2: Core Integration Layer (HIGH PRIORITY)

### 2.1 github_integration.py
**Status:** NOT STARTED - 0% coverage
**Target:** 80% coverage (~40 tests)
**Test File:** NEW - `backend/tests/test_github_integration.py`

**Test Categories:**

#### GitHubReleasesFetcher (25 tests)
```python
# Initialization (2 tests)
- test_init_sets_metadata_manager_and_ttl()
- test_init_creates_cache_structures()

# Cache Management (8 tests)
- test_get_releases_uses_memory_cache_when_valid()
- test_get_releases_uses_disk_cache_when_memory_expired()
- test_get_releases_fetches_from_github_when_no_cache()
- test_get_releases_force_refresh_bypasses_cache()
- test_get_releases_returns_stale_cache_when_offline()
- test_get_releases_returns_empty_when_no_cache_offline()
- test_is_cache_valid_returns_true_when_within_ttl()
- test_is_cache_valid_returns_false_when_expired()

# GitHub API Fetching (5 tests)
- test_fetch_page_success_with_retries()
- test_fetch_page_handles_rate_limit_403()
- test_fetch_page_retries_on_network_error()
- test_fetch_page_uses_exponential_backoff()
- test_fetch_from_github_paginates_releases()

# Release Operations (5 tests)
- test_get_latest_release_excludes_prerelease()
- test_get_latest_release_includes_prerelease()
- test_get_release_by_tag_found()
- test_get_release_by_tag_not_found()
- test_collapse_latest_patch_per_minor()

# Cache Status (3 tests)
- test_get_cache_status_with_valid_cache()
- test_get_cache_status_with_stale_cache()
- test_get_cache_status_with_no_cache()

# Error Handling (2 tests)
- test_get_releases_handles_json_decode_error()
- test_get_releases_handles_network_timeout()
```

#### DownloadManager (15 tests)
```python
# Download Operations (6 tests)
- test_download_file_success()
- test_download_file_with_progress_callback()
- test_download_file_creates_parent_directories()
- test_download_file_handles_network_error()
- test_download_file_handles_disk_full_error()
- test_download_file_cleans_up_partial_download()

# Cancellation (3 tests)
- test_download_file_cancellation_requested()
- test_cancel_stops_active_download()
- test_cancel_cleans_up_partial_file()

# Retry Logic (4 tests)
- test_download_with_retry_succeeds_on_first_attempt()
- test_download_with_retry_succeeds_on_second_attempt()
- test_download_with_retry_fails_after_max_retries()
- test_download_with_retry_uses_exponential_backoff()

# Progress Tracking (2 tests)
- test_download_progress_callback_receives_speed()
- test_download_progress_callback_throttling()
```

**Mocking Strategy:**
```python
@pytest.fixture
def mock_urllib_response(mocker):
    """Mock urllib.request.urlopen for controlled responses"""
    mock_response = mocker.MagicMock()
    mock_response.headers.get.return_value = "1024"
    mock_response.read.side_effect = [b"data", b""]
    return mock_response

@pytest.fixture
def mock_metadata_manager(mocker):
    """Mock MetadataManager for cache operations"""
    mock = mocker.MagicMock()
    mock.load_github_cache.return_value = None
    return mock
```

### 2.2 version_manager.py
**Status:** NOT STARTED - 0% coverage
**Target:** 75% coverage (~35 tests)
**Test File:** NEW - `backend/tests/test_version_manager.py`

**Test Categories:**

#### Initialization (5 tests)
```python
- test_init_creates_versions_directory()
- test_init_loads_constraints_cache()
- test_init_initializes_progress_tracker()
- test_init_sets_active_version_from_file()
- test_initialize_active_version_priority_rules()
```

#### Release Management (8 tests)
```python
- test_get_available_releases_without_collapse()
- test_get_available_releases_with_collapse()
- test_get_available_releases_excludes_prerelease()
- test_get_available_releases_force_refresh()
- test_get_installed_versions_returns_list()
- test_get_version_path_for_installed_version()
- test_get_version_path_for_missing_version()
- test_get_version_info_returns_metadata()
```

#### Active Version Management (5 tests)
```python
- test_get_active_version_from_memory()
- test_get_active_version_from_file()
- test_set_active_version_writes_file()
- test_set_active_version_updates_memory()
- test_clear_active_version()
```

#### Installation Operations (10 tests - focus on mocking, not full integration)
```python
- test_install_version_creates_directory_structure()
- test_install_version_downloads_archive()
- test_install_version_creates_venv()
- test_install_version_installs_dependencies()
- test_install_version_updates_progress_tracker()
- test_install_version_handles_cancellation()
- test_install_version_cleans_up_on_failure()
- test_cancel_installation_sets_flag()
- test_cancel_installation_kills_process()
- test_cancel_installation_cancels_download()
```

#### Uninstallation (3 tests)
```python
- test_uninstall_version_removes_directory()
- test_uninstall_version_clears_active_if_current()
- test_uninstall_version_handles_missing_version()
```

#### Constraints & Dependencies (4 tests)
```python
- test_get_constraints_for_version()
- test_load_constraints_cache()
- test_save_constraints_cache()
- test_resolve_dependencies_from_requirements()
```

**Mocking Strategy:**
```python
@pytest.fixture
def version_manager(tmp_path, mock_metadata_manager, mock_github_fetcher, mock_resource_manager):
    """Create VersionManager with mocked dependencies"""
    return VersionManager(
        launcher_root=tmp_path,
        metadata_manager=mock_metadata_manager,
        github_fetcher=mock_github_fetcher,
        resource_manager=mock_resource_manager
    )

@pytest.fixture
def mock_subprocess(mocker):
    """Mock subprocess operations for venv creation"""
    return mocker.patch('subprocess.run')
```

### 2.3 core.py (ComfyUISetupAPI)
**Status:** MINIMAL - 15% coverage
**Target:** 70% coverage (~50 tests)
**Test File:** `backend/tests/test_core_api.py` (expand existing)

**Test Categories:**

#### Initialization (8 tests)
```python
- test_init_in_development_mode()
- test_init_in_frozen_mode()
- test_find_comfyui_root_searches_parent_dirs()
- test_find_comfyui_root_returns_none_when_not_found()
- test_init_version_management_success()
- test_init_version_management_without_version_manager()
- test_init_managers_creates_all_components()
- test_prefetch_releases_spawns_background_thread()
```

#### Rate Limiting (4 tests)
```python
- test_is_rate_limited_returns_false_initially()
- test_is_rate_limited_returns_true_within_window()
- test_is_rate_limited_resets_after_window()
- test_is_rate_limited_per_operation()
```

#### Dependency Methods (5 tests)
```python
- test_check_deps_returns_status()
- test_get_missing_deps_filters_correctly()
- test_install_deps_delegates_to_manager()
- test_check_setproctitle_status()
- test_check_git_status()
```

#### Process Management (6 tests)
```python
- test_get_status_delegates_to_system_utils()
- test_launch_comfyui_delegates_to_process_manager()
- test_stop_comfyui_delegates_to_process_manager()
- test_is_comfyui_running_returns_boolean()
- test_get_processes_with_resources()
- test_get_disk_space_delegates_to_system_utils()
```

#### Version Management Wrappers (12 tests)
```python
- test_get_available_releases_wrapper()
- test_get_installed_versions_wrapper()
- test_get_active_version_wrapper()
- test_set_active_version_wrapper()
- test_install_version_wrapper()
- test_uninstall_version_wrapper()
- test_get_version_info_wrapper()
- test_get_version_path_wrapper()
- test_cancel_installation_wrapper()
- test_get_installation_progress_wrapper()
- test_get_cache_status_wrapper()
- test_refresh_releases_wrapper()
```

#### Shortcut Management Wrappers (10 tests)
```python
- test_toggle_menu_delegates_correctly()
- test_toggle_desktop_delegates_correctly()
- test_get_version_shortcuts_wrapper()
- test_get_all_shortcut_states_wrapper()
- test_create_version_shortcuts_wrapper()
- test_remove_version_shortcuts_wrapper()
- test_set_version_shortcuts_wrapper()
- test_toggle_version_menu_wrapper()
- test_toggle_version_desktop_wrapper()
- test_install_icon_wrapper()
```

#### Patch Management (3 tests)
```python
- test_is_patched_wrapper()
- test_patch_main_py_wrapper()
- test_revert_main_py_wrapper()
```

#### Resource & Size Wrappers (2 tests)
```python
- test_get_system_resources_wrapper()
- test_calculate_release_size_wrapper()
```

### 2.4 main.py (JavaScriptAPI)
**Status:** MINIMAL - 25% coverage
**Target:** 75% coverage (~45 tests)
**Test File:** `backend/tests/test_main_api.py` (expand existing)

**Test Categories:**

#### Initialization (3 tests)
```python
- test_javascript_api_init_creates_setup_api()
- test_javascript_api_exposes_all_methods()
- test_close_window_performs_cleanup()
```

#### Error Wrapping (8 tests)
```python
- test_methods_catch_exceptions_and_return_errors()
- test_methods_log_exceptions()
- test_methods_return_success_on_normal_operation()
- test_exception_to_dict_conversion()
- test_network_errors_have_retry_hints()
- test_validation_errors_include_field_info()
- test_rate_limit_errors_include_wait_time()
- test_concurrent_operation_errors()
```

#### Process Operations (6 tests)
```python
- test_get_status_returns_formatted_response()
- test_launch_comfyui_success()
- test_launch_comfyui_error_handling()
- test_stop_comfyui_success()
- test_stop_comfyui_error_handling()
- test_is_comfyui_running_wrapper()
```

#### Version Operations (10 tests)
```python
- test_get_available_releases_wrapper()
- test_get_installed_versions_wrapper()
- test_install_version_wrapper()
- test_uninstall_version_wrapper()
- test_set_active_version_wrapper()
- test_get_active_version_wrapper()
- test_cancel_installation_wrapper()
- test_get_installation_progress_wrapper()
- test_get_version_info_wrapper()
- test_refresh_releases_wrapper()
```

#### Shortcut Operations (8 tests)
```python
- test_toggle_menu_wrapper()
- test_toggle_desktop_wrapper()
- test_get_all_shortcut_states_wrapper()
- test_create_version_shortcuts_wrapper()
- test_remove_version_shortcuts_wrapper()
- test_set_version_shortcuts_wrapper()
- test_toggle_version_menu_wrapper()
- test_toggle_version_desktop_wrapper()
```

#### Resource & System Operations (6 tests)
```python
- test_get_system_resources_wrapper()
- test_get_disk_space_wrapper()
- test_open_path_wrapper()
- test_open_url_wrapper()
- test_open_active_install_wrapper()
- test_calculate_release_size_wrapper()
```

#### Miscellaneous (4 tests)
```python
- test_check_deps_wrapper()
- test_install_deps_wrapper()
- test_toggle_patch_wrapper()
- test_get_cache_status_wrapper()
```

---

## Phase 3: Supporting Modules (MEDIUM PRIORITY)

### 3.1 shortcut_manager.py
**Status:** NOT STARTED - 0% coverage
**Target:** 75% coverage (~30 tests)
**Test File:** NEW - `backend/tests/test_shortcut_manager.py`

**Test Categories:**

#### Initialization & Utilities (5 tests)
```python
- test_init_sets_paths_correctly()
- test_slugify_tag_handles_special_characters()
- test_slugify_tag_strips_leading_trailing()
- test_get_version_paths_returns_dict_when_valid()
- test_get_version_paths_returns_none_when_invalid()
```

#### Icon Generation (8 tests)
```python
- test_generate_version_icon_success()
- test_generate_version_icon_without_pillow()
- test_generate_version_icon_missing_base_icon()
- test_validate_icon_prerequisites()
- test_create_icon_canvas()
- test_draw_version_banner()
- test_save_generated_icon()
- test_remove_installed_icon()
```

#### Icon Installation (5 tests)
```python
- test_install_icon_success_with_imagemagick()
- test_install_icon_fallback_without_imagemagick()
- test_install_icon_updates_icon_cache()
- test_install_version_icon_with_overlay()
- test_install_version_icon_fallback_to_base()
```

#### Launch Script Generation (4 tests)
```python
- test_write_version_launch_script_creates_file()
- test_write_version_launch_script_content()
- test_write_version_launch_script_permissions()
- test_write_version_launch_script_error_handling()
```

#### Shortcut State (3 tests)
```python
- test_get_version_shortcut_state()
- test_get_all_shortcut_states()
- test_menu_exists_and_desktop_exists()
```

#### Shortcut Creation (3 tests)
```python
- test_create_version_shortcuts_menu_only()
- test_create_version_shortcuts_desktop_only()
- test_create_version_shortcuts_both()
```

#### Shortcut Removal (2 tests)
```python
- test_remove_version_shortcuts()
- test_remove_version_shortcuts_cleans_launcher_script()
```

### 3.2 metadata_manager.py
**Status:** NOT STARTED - 0% coverage
**Target:** 80% coverage (~25 tests)
**Test File:** NEW - `backend/tests/test_metadata_manager.py`

**Test Categories:**

#### Initialization (3 tests)
```python
- test_init_creates_directory_structure()
- test_init_sets_paths_correctly()
- test_ensure_data_directories()
```

#### GitHub Cache (6 tests)
```python
- test_save_github_cache_writes_json()
- test_load_github_cache_reads_json()
- test_load_github_cache_returns_none_when_missing()
- test_load_github_cache_handles_corrupted_json()
- test_github_cache_ttl_validation()
- test_clear_github_cache()
```

#### Version Metadata (6 tests)
```python
- test_save_version_metadata()
- test_load_version_metadata()
- test_load_version_metadata_missing_version()
- test_update_version_metadata()
- test_delete_version_metadata()
- test_list_all_version_metadata()
```

#### Installation Progress (4 tests)
```python
- test_save_installation_progress()
- test_load_installation_progress()
- test_clear_installation_progress()
- test_installation_progress_file_structure()
```

#### Size Cache (3 tests)
```python
- test_save_size_cache()
- test_load_size_cache()
- test_size_cache_expiration()
```

#### General File Operations (3 tests)
```python
- test_atomic_write_prevents_corruption()
- test_file_locking_during_write()
- test_permission_error_handling()
```

### 3.3 size_calculator.py
**Status:** NOT STARTED - 0% coverage
**Target:** 75% coverage (~20 tests)
**Test File:** NEW - `backend/tests/test_size_calculator.py`

**Test Categories:**

#### Initialization (2 tests)
```python
- test_init_sets_dependencies()
- test_init_without_version_manager()
```

#### Release Size Calculation (6 tests)
```python
- test_calculate_release_size_with_cache()
- test_calculate_release_size_without_cache()
- test_calculate_release_size_force_refresh()
- test_calculate_release_size_handles_missing_release()
- test_calculate_release_size_network_error()
- test_get_content_length_success()
```

#### Batch Operations (3 tests)
```python
- test_calculate_all_release_sizes()
- test_calculate_all_release_sizes_with_progress()
- test_calculate_all_release_sizes_without_version_manager()
```

#### Background Refresh (3 tests)
```python
- test_refresh_release_sizes_async_prioritizes_non_installed()
- test_refresh_release_sizes_async_skips_cached()
- test_refresh_release_sizes_async_handles_errors()
```

#### Size Info & Breakdown (4 tests)
```python
- test_get_release_size_info()
- test_get_release_size_breakdown()
- test_get_release_dependencies()
- test_get_release_dependencies_top_n()
```

#### Error Handling (2 tests)
```python
- test_handles_urllib_errors()
- test_handles_missing_calculator()
```

### 3.4 version_info.py
**Status:** NOT STARTED - 0% coverage
**Target:** 80% coverage (~15 tests)
**Test File:** NEW - `backend/tests/test_version_info.py`

**Test Categories:**

#### Initialization (2 tests)
```python
- test_init_with_github_fetcher()
- test_init_without_github_fetcher()
```

#### Version Detection (6 tests)
```python
- test_get_comfyui_version_from_pyproject()
- test_get_comfyui_version_from_git_describe()
- test_get_comfyui_version_from_github_api()
- test_get_comfyui_version_fallback_to_unknown()
- test_get_comfyui_version_handles_toml_error()
- test_get_comfyui_version_priority_order()
```

#### Release Checking (5 tests)
```python
- test_check_for_new_release_update_available()
- test_check_for_new_release_up_to_date()
- test_check_for_new_release_force_refresh()
- test_check_for_new_release_uses_cache()
- test_check_for_new_release_handles_errors()
```

#### Edge Cases (2 tests)
```python
- test_handles_git_not_installed()
- test_handles_network_unavailable()
```

### 3.5 resource_manager.py
**Status:** NOT STARTED - 0% coverage
**Target:** 70% coverage (~25 tests)
**Test File:** NEW - `backend/tests/test_resource_manager.py`

**Test Categories:**

#### Initialization (3 tests)
```python
- test_init_creates_storage_directory()
- test_init_loads_shared_storage_config()
- test_init_without_version_manager()
```

#### Storage Management (8 tests)
```python
- test_get_storage_path_for_resource_type()
- test_set_storage_path_creates_directory()
- test_validate_storage_path()
- test_ensure_storage_exists()
- test_get_storage_usage()
- test_storage_path_with_symlinks()
- test_storage_path_validation_errors()
- test_storage_cleanup_operations()
```

#### Model Management (6 tests)
```python
- test_list_models_by_type()
- test_download_model()
- test_delete_model()
- test_model_exists_check()
- test_get_model_info()
- test_scan_for_models()
```

#### Custom Nodes (4 tests)
```python
- test_list_custom_nodes()
- test_install_custom_node()
- test_remove_custom_node()
- test_update_custom_node()
```

#### Symlink Management (4 tests)
```python
- test_create_symlink_for_shared_resource()
- test_remove_symlink()
- test_verify_symlink_integrity()
- test_repair_broken_symlinks()
```

---

## Phase 4: Utilities & Helpers (LOWER PRIORITY)

### 4.1 validators.py
**Status:** NOT STARTED - 0% coverage
**Target:** 90% coverage (~15 tests)
**Test File:** NEW - `backend/tests/test_validators.py`

```python
# URL Validation (4 tests)
- test_validate_url_valid_http()
- test_validate_url_valid_https()
- test_validate_url_invalid_scheme()
- test_validate_url_malformed()

# Path Validation (4 tests)
- test_validate_path_valid_absolute()
- test_validate_path_valid_relative()
- test_validate_path_invalid_characters()
- test_validate_path_traversal_attack()

# Version Tag Validation (3 tests)
- test_validate_version_tag_semver()
- test_validate_version_tag_with_v_prefix()
- test_validate_version_tag_invalid()

# String Validation (2 tests)
- test_sanitize_string_removes_dangerous_chars()
- test_validate_string_length()

# Port Validation (2 tests)
- test_validate_port_number_valid()
- test_validate_port_number_invalid()
```

### 4.2 utils.py
**Status:** NOT STARTED - 0% coverage
**Target:** 70% coverage (~20 tests)
**Test File:** NEW - `backend/tests/test_utils.py`

```python
# Path Operations (6 tests)
- test_get_launcher_root()
- test_ensure_directory_creates_path()
- test_ensure_directory_handles_existing()
- test_safe_path_join_prevents_traversal()
- test_expand_user_path()
- test_resolve_symlinks()

# File Operations (4 tests)
- test_atomic_write()
- test_safe_delete_file()
- test_copy_with_progress()
- test_calculate_directory_size()

# String Operations (4 tests)
- test_format_bytes_various_sizes()
- test_format_duration()
- test_truncate_string()
- test_slugify()

# System Operations (3 tests)
- test_get_platform_info()
- test_is_running_in_venv()
- test_get_python_version()

# Miscellaneous (3 tests)
- test_retry_decorator()
- test_timeout_decorator()
- test_parse_config_value()
```

### 4.3 file_utils.py
**Status:** NOT STARTED - 0% coverage
**Target:** 80% coverage (~12 tests)
**Test File:** NEW - `backend/tests/test_file_utils.py`

```python
# Archive Operations (4 tests)
- test_extract_archive_zip()
- test_extract_archive_tar_gz()
- test_extract_archive_with_progress()
- test_extract_archive_handles_errors()

# File Search (3 tests)
- test_find_files_by_pattern()
- test_find_files_recursive()
- test_find_files_with_exclude()

# File Comparison (2 tests)
- test_files_identical()
- test_calculate_file_hash()

# File Permissions (3 tests)
- test_make_executable()
- test_set_permissions()
- test_check_writable()
```

### 4.4 models.py
**Status:** NOT STARTED - 0% coverage
**Target:** 70% coverage (~18 tests)
**Test File:** NEW - `backend/tests/test_models.py`

```python
# Timestamp Operations (4 tests)
- test_get_iso_timestamp()
- test_parse_iso_timestamp()
- test_parse_iso_timestamp_invalid()
- test_timestamp_round_trip()

# Model Validation (6 tests)
- test_github_release_schema()
- test_release_schema()
- test_github_releases_cache_schema()
- test_version_metadata_schema()
- test_installation_progress_schema()
- test_process_info_schema()

# Model Conversion (4 tests)
- test_github_release_to_release()
- test_process_info_to_dict()
- test_version_metadata_defaults()
- test_cache_data_serialization()

# Type Guards (4 tests)
- test_is_github_release()
- test_is_release()
- test_is_process_info()
- test_is_installation_progress()
```

### 4.5 release_size_calculator.py
**Status:** NOT STARTED - 0% coverage
**Target:** 65% coverage (~20 tests)
**Test File:** NEW - `backend/tests/test_release_size_calculator.py`

```python
# Initialization (2 tests)
- test_init_loads_cache()
- test_init_creates_cache_directory()

# Size Calculation (6 tests)
- test_calculate_release_size_from_cache()
- test_calculate_release_size_computes_new()
- test_calculate_release_size_includes_dependencies()
- test_calculate_release_size_includes_venv()
- test_calculate_release_size_force_refresh()
- test_calculate_release_size_handles_errors()

# Dependency Resolution (4 tests)
- test_resolve_dependency_sizes()
- test_resolve_dependency_sizes_with_constraints()
- test_get_sorted_dependencies()
- test_dependency_size_estimation()

# Size Breakdown (3 tests)
- test_get_size_breakdown_structure()
- test_size_breakdown_components()
- test_size_breakdown_formatting()

# Cache Management (3 tests)
- test_get_cached_size()
- test_save_to_cache()
- test_cache_expiration()

# Error Handling (2 tests)
- test_handles_network_errors()
- test_handles_missing_requirements()
```

---

## Phase 5: Advanced & Optional Modules

### 5.1 installation_progress_tracker.py
**Target:** 70% coverage (~12 tests)

```python
# Progress Tracking (4 tests)
- test_start_installation_creates_tracker()
- test_update_progress()
- test_get_progress()
- test_complete_installation()

# State Management (4 tests)
- test_progress_states()
- test_cancel_installation()
- test_installation_failure()
- test_concurrent_installations()

# Persistence (4 tests)
- test_save_progress_to_disk()
- test_load_progress_from_disk()
- test_cleanup_old_progress()
- test_progress_history()
```

### 5.2 launcher_updater.py
**Target:** 65% coverage (~10 tests)

```python
# Update Checking (3 tests)
- test_check_for_updates()
- test_compare_versions()
- test_update_available_notification()

# Update Installation (4 tests)
- test_download_update()
- test_apply_update()
- test_rollback_update()
- test_update_verification()

# Error Handling (3 tests)
- test_handles_network_errors()
- test_handles_permission_errors()
- test_handles_corrupted_download()
```

### 5.3 package_size_resolver.py
**Target:** 70% coverage (~15 tests)

```python
# Size Resolution (5 tests)
- test_get_package_size_from_pypi()
- test_get_package_size_from_cache()
- test_estimate_package_size()
- test_batch_size_resolution()
- test_handle_package_not_found()

# Wheel Analysis (4 tests)
- test_parse_wheel_metadata()
- test_estimate_installed_size()
- test_wheel_platform_compatibility()
- test_wheel_dependency_resolution()

# Cache Management (3 tests)
- test_cache_package_sizes()
- test_cache_ttl()
- test_clear_cache()

# Error Handling (3 tests)
- test_handles_pypi_timeout()
- test_handles_invalid_package_name()
- test_handles_metadata_errors()
```

### 5.4 rate_limiter.py
**Target:** 85% coverage (~10 tests)

```python
# Rate Limiting (4 tests)
- test_rate_limiter_allows_within_limit()
- test_rate_limiter_blocks_over_limit()
- test_rate_limiter_resets_after_window()
- test_rate_limiter_per_key()

# Token Bucket (3 tests)
- test_token_bucket_refill()
- test_token_bucket_consume()
- test_token_bucket_burst()

# Decorators (3 tests)
- test_rate_limit_decorator()
- test_rate_limit_decorator_async()
- test_rate_limit_decorator_error_handling()
```

### 5.5 logging_config.py
**Target:** 75% coverage (~8 tests)

```python
# Logger Setup (3 tests)
- test_get_logger_creates_logger()
- test_logger_levels_configuration()
- test_logger_formatting()

# File Handlers (2 tests)
- test_file_handler_creation()
- test_log_rotation()

# Console Handlers (2 tests)
- test_console_handler_creation()
- test_color_formatting()

# Configuration (1 test)
- test_configure_logging()
```

### 5.6 exceptions.py
**Target:** 100% coverage (~6 tests)

```python
# Custom Exceptions (6 tests)
- test_network_error_creation()
- test_metadata_error_creation()
- test_validation_error_creation()
- test_installation_error_creation()
- test_exception_inheritance()
- test_exception_message_formatting()
```

---

## Test Infrastructure & Best Practices

### Shared Fixtures (conftest.py)

```python
# Temporary Directories
@pytest.fixture
def temp_launcher_root(tmp_path):
    """Temporary launcher root directory"""
    root = tmp_path / "launcher"
    root.mkdir()
    return root

# Mock Managers
@pytest.fixture
def mock_metadata_manager(mocker):
    """Mock MetadataManager"""
    manager = mocker.MagicMock()
    manager.launcher_data_dir = Path("/tmp/launcher-data")
    manager.cache_dir = Path("/tmp/cache")
    return manager

@pytest.fixture
def mock_github_fetcher(mocker):
    """Mock GitHubReleasesFetcher"""
    fetcher = mocker.MagicMock()
    fetcher.get_releases.return_value = []
    return fetcher

@pytest.fixture
def mock_version_manager(mocker):
    """Mock VersionManager"""
    manager = mocker.MagicMock()
    manager.get_installed_versions.return_value = []
    manager.get_active_version.return_value = None
    return manager

# Sample Data
@pytest.fixture
def sample_github_release():
    """Sample GitHub release data"""
    return {
        "tag_name": "v0.2.0",
        "name": "Version 0.2.0",
        "published_at": "2024-01-01T00:00:00Z",
        "body": "Release notes",
        "zipball_url": "https://example.com/archive.zip",
        "prerelease": False
    }

@pytest.fixture
def sample_process_info():
    """Sample process info"""
    return {
        "pid": 12345,
        "name": "python",
        "cpu": 50.0,
        "ram": 512.0,
        "gpu_ram": 1024.0,
        "version": "v0.2.0"
    }
```

### Testing Patterns

#### 1. Parametrized Tests
```python
@pytest.mark.parametrize("tag,expected", [
    ("v0.1.0", "v0-1-0"),
    ("v0.2.0-beta", "v0-2-0-beta"),
    ("invalid@tag!", "invalid-tag-"),
])
def test_slugify_tag(tag, expected):
    manager = ShortcutManager(...)
    assert manager._slugify_tag(tag) == expected
```

#### 2. Exception Testing
```python
def test_method_raises_exception_on_invalid_input():
    with pytest.raises(ValueError, match="Invalid input"):
        some_function(invalid_input)
```

#### 3. Mock Assertions
```python
def test_method_calls_dependency(mocker):
    mock_dependency = mocker.patch('module.dependency')
    some_function()
    mock_dependency.assert_called_once_with(expected_args)
```

#### 4. Temporary File Testing
```python
def test_writes_file_correctly(tmp_path):
    file_path = tmp_path / "test.txt"
    write_function(file_path, "content")
    assert file_path.read_text() == "content"
```

### Coverage Reporting

```bash
# Generate HTML coverage report
pytest --cov=backend --cov-report=html --cov-report=term-missing backend/tests/

# View coverage report
open htmlcov/index.html  # macOS/Linux
start htmlcov/index.html # Windows

# Coverage summary
pytest --cov=backend --cov-report=term backend/tests/

# Coverage for specific module
pytest --cov=backend.api.process_manager --cov-report=term backend/tests/test_process_manager.py
```

### CI/CD Integration

```yaml
# .github/workflows/tests.yml
name: Unit Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'
      - run: pip install -r requirements.txt -r requirements-dev.txt
      - run: pytest --cov=backend --cov-report=xml --cov-report=term
      - uses: codecov/codecov-action@v3
        with:
          files: ./coverage.xml
```

---

## Success Metrics

### Overall Targets
- ✅ **80%+ line coverage** across all backend modules
- ✅ **75%+ branch coverage** for critical paths
- ✅ **90%+ function coverage** for public APIs
- ✅ **Zero critical uncovered paths** in error handling

### Per-Module Targets
- **P0 Modules:** 85-95% coverage (critical path)
- **P1 Modules:** 75-85% coverage (core features)
- **P2 Modules:** 70-80% coverage (supporting features)
- **P3-P4 Modules:** 60-75% coverage (utilities)

### Quality Gates
- All tests must pass before merge
- No regressions in existing test coverage
- New features require tests achieving module target coverage
- All public methods must have at least one test
- Critical error paths must be explicitly tested

---

## Implementation Timeline

### Weeks 1-2: Complete Phase 2 Foundation
- Expand process_manager.py tests (+20 tests)
- Expand system_utils.py tests (+25 tests)
- Finalize dependency_manager.py (+5 tests)
- Finalize patch_manager.py (+5 tests)
- **Milestone:** Core modules at 85%+ coverage

### Weeks 3-4: Core Integration Layer
- Implement github_integration.py tests (+40 tests)
- Implement version_manager.py tests (+35 tests)
- **Milestone:** Integration layer at 75%+ coverage

### Weeks 5-6: API & Supporting Modules
- Expand core.py tests (+50 tests)
- Expand main.py tests (+45 tests)
- Implement shortcut_manager.py tests (+30 tests)
- Implement metadata_manager.py tests (+25 tests)
- **Milestone:** APIs at 70%+ coverage

### Weeks 7-8: Utilities & Advanced Features
- Implement size_calculator.py tests (+20 tests)
- Implement version_info.py tests (+15 tests)
- Implement resource_manager.py tests (+25 tests)
- Implement validators.py tests (+15 tests)
- Implement utils.py tests (+20 tests)
- **Milestone:** 80%+ overall coverage achieved

### Weeks 9-10: Polish & Documentation
- Implement remaining utility tests
- Achieve 85%+ on critical modules
- Document testing patterns
- Create test maintenance guide
- **Milestone:** Production-ready test suite

---

## Maintenance & Evolution

### Regular Activities
- Weekly: Review coverage reports, identify gaps
- Monthly: Refactor duplicate test code
- Quarterly: Update test fixtures for new features
- Annually: Review and modernize testing patterns

### When Adding New Features
1. Write tests first (TDD) or alongside implementation
2. Achieve module target coverage (70-90%)
3. Add integration tests if crossing modules
4. Update conftest.py with new fixtures
5. Document any new testing patterns

### When Fixing Bugs
1. Write failing test that reproduces bug
2. Fix the bug
3. Verify test passes
4. Add related edge case tests
5. Update coverage targets if needed

---

## Conclusion

This comprehensive test plan provides a roadmap to achieve 80%+ code coverage across the ComfyUI Launcher backend. By prioritizing critical modules first and following structured testing patterns, we ensure:

1. **Reliability** - Catching bugs before production
2. **Maintainability** - Tests as living documentation
3. **Confidence** - Safe refactoring and feature additions
4. **Quality** - Consistent code quality standards

**Current Status:** 112 tests passing, ~30% coverage
**Final Goal:** 450-500 tests passing, 80%+ coverage
**Timeline:** 8-10 weeks with dedicated effort

The test suite will serve as both a safety net for refactoring and a specification for expected behavior, making the codebase more maintainable and robust over time.
