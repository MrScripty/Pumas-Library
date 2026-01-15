/**
 * Installation Formatters Utility
 *
 * Formatting functions for installation progress and version display.
 * Extracted from InstallDialog.tsx
 */

/**
 * Format bytes to human-readable size
 */
export function formatSize(bytes: number | null | undefined): string {
  if (!bytes || bytes === 0) return '';

  if (bytes < 1024) {
    return `${bytes} B`;
  } else if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  } else if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  } else {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
}

/**
 * Format bytes to GB
 */
export function formatGB(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '0.00 GB';
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Format ETA in seconds to human-readable time
 */
export function formatETA(seconds: number): string {
  const maxEtaSeconds = 48 * 3600 + 59 * 60;
  const clampedSeconds = Math.min(seconds, maxEtaSeconds);
  if (clampedSeconds < 60) return `${Math.round(clampedSeconds)}s`;
  if (clampedSeconds < 3600) return `${Math.floor(clampedSeconds / 60)}m ${Math.round(clampedSeconds % 60)}s`;
  return `${Math.floor(clampedSeconds / 3600)}h ${Math.floor((clampedSeconds % 3600) / 60)}m`;
}

/**
 * Format elapsed time from start timestamp
 */
export function formatElapsedTime(startedAt: string): string {
  const start = new Date(startedAt);
  const now = new Date();
  const elapsed = Math.floor((now.getTime() - start.getTime()) / 1000);
  return formatETA(elapsed);
}

/**
 * Format date to readable format
 */
export function formatVersionDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric'
  });
}

/**
 * Get GitHub release URL for a version
 */
export function getReleaseUrl(release: { html_url?: string; tag_name: string }): string {
  return release.html_url || '';
}
