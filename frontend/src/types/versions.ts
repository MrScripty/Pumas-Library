/**
 * Version Management Type Definitions
 *
 * Shared types for version management, installation, and status.
 * Extracted from hooks/useVersions.ts
 */

export interface VersionRelease {
  tagName: string;
  name: string;
  publishedAt: string;
  prerelease: boolean;
  body?: string;
  htmlUrl?: string;
  totalSize?: number | null;
  archiveSize?: number | null;
  dependenciesSize?: number | null;
  installing?: boolean;
  /** Why the backend cannot install this build on this host; null when it can. */
  unavailableReason?: string | null;
}

export type VersionStatus = import('@pumas/desktop-contract').VersionStatusOutcome['status'];

export type VersionInfo = import('@pumas/desktop-contract').VersionInfoOutcome['info'];

export interface InstallationProgress {
  tag: string;
  started_at: string;
  stage: 'resolving' | 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress: number;
  overall_progress: number;
  current_item: string | null;
  download_source_url?: string | null;
  download_active?: boolean;
  download_measurement_available?: boolean | null;
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

export type CacheStatus = import('@pumas/desktop-contract').GithubCacheStatusOutcome;
