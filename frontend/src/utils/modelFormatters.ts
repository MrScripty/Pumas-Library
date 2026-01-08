/**
 * Model formatting utilities
 *
 * Helper functions for formatting model data.
 * Extracted from ModelManager.tsx
 */

import {
  Calendar,
  CalendarCheck,
  CalendarFold,
  Calendars,
} from 'lucide-react';

/**
 * Format file size in bytes to human-readable string
 */
export function formatSize(bytes?: number): string {
  if (!bytes) return 'Unknown';
  const gb = bytes / (1024 ** 3);
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  const mb = bytes / (1024 ** 2);
  return `${mb.toFixed(2)} MB`;
}

/**
 * Format date string to localized date
 */
export function formatDate(dateStr?: string): string {
  if (!dateStr) return 'Unknown';
  try {
    return new Date(dateStr).toLocaleDateString();
  } catch {
    return 'Unknown';
  }
}

/**
 * Get appropriate calendar icon based on release date
 */
export function resolveReleaseIcon(dateStr?: string) {
  if (!dateStr) return Calendar;
  const parsed = new Date(dateStr);
  if (Number.isNaN(parsed.getTime())) {
    return Calendar;
  }
  const now = new Date();
  const diffMs = now.getTime() - parsed.getTime();
  const diffDays = diffMs / (1000 * 60 * 60 * 24);
  if (diffDays <= 60) {
    return CalendarCheck;
  }
  if (diffDays <= 240) {
    return CalendarFold;
  }
  return Calendars;
}

/**
 * Format release date
 */
export function formatReleaseDate(dateStr?: string): string {
  if (!dateStr) return 'Unknown';
  const parsed = new Date(dateStr);
  if (Number.isNaN(parsed.getTime())) {
    return 'Unknown';
  }
  return parsed.toLocaleDateString();
}

/**
 * Format download count
 */
export function formatDownloads(downloads?: number | null): string {
  if (typeof downloads !== 'number') {
    return 'Unknown';
  }
  return downloads.toLocaleString();
}

/**
 * Format download size in GB
 */
export function formatDownloadSize(bytes?: number | null): string {
  if (typeof bytes !== 'number' || bytes <= 0) {
    return 'Unknown';
  }
  const gb = bytes / (1024 ** 3);
  const rounded = gb >= 10 ? gb.toFixed(1) : gb.toFixed(2);
  return `${rounded} GB`;
}

/**
 * Get timestamp from release date for sorting
 */
export function getReleaseTimestamp(date?: string): number {
  if (!date) return 0;
  const parsed = new Date(date);
  const time = parsed.getTime();
  return Number.isNaN(time) ? 0 : time;
}
