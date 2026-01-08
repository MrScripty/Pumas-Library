/**
 * Version Management Type Definitions
 *
 * Shared types for version management, installation, and status.
 * Extracted from hooks/useVersions.ts
 */

export interface VersionRelease {
  tag_name: string;
  name: string;
  published_at: string;
  prerelease: boolean;
  body?: string;
  html_url?: string;
  total_size?: number | null;
  archive_size?: number | null;
  dependencies_size?: number | null;
  installing?: boolean;
}

export interface VersionStatus {
  installedCount: number;
  activeVersion: string | null;
  defaultVersion?: string | null;
  versions: {
    [tag: string]: {
      isActive: boolean;
      dependencies: {
        installed: string[];
        missing: string[];
      };
    };
  };
}

export interface VersionInfo {
  path: string;
  installedDate: string;
  pythonVersion: string;
  releaseTag: string;
}

export interface InstallationProgress {
  tag: string;
  started_at: string;
  stage: 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress: number;
  overall_progress: number;
  current_item: string | null;
  download_speed: number | null;
  eta_seconds: number | null;
  total_size: number | null;
  downloaded_bytes: number;
  dependency_count: number | null;
  completed_dependencies: number;
  completed_items: Array<{
    name: string;
    type: string;
    size: number | null;
    completed_at: string;
  }>;
  error: string | null;
  completed_at?: string;
  success?: boolean;
  log_path?: string | null;
}

export type InstallNetworkStatus = 'idle' | 'downloading' | 'stalled' | 'failed';

export interface CacheStatus {
  has_cache: boolean;
  is_valid: boolean;
  is_fetching: boolean;
  age_seconds?: number;
  last_fetched?: string;
  releases_count?: number;
}
