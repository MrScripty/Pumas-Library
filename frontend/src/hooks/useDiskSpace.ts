/**
 * Disk space monitoring hook
 *
 * Reads disk usage on demand. An unread or failed reading is `null` with a
 * reason, never a 0% reading.
 */

import { useState, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useDiskSpace');

export function useDiskSpace() {
  const [diskSpacePercent, setDiskSpacePercent] = useState<number | null>(null);
  const [diskSpaceError, setDiskSpaceError] = useState<string | null>(null);

  const recordFailure = useCallback((reason: string) => {
    setDiskSpacePercent(null);
    setDiskSpaceError(reason);
  }, []);

  const fetchDiskSpace = useCallback(async () => {
    if (!isAPIAvailable()) {
      recordFailure('The desktop backend is not available.');
      return;
    }
    try {
      const diskData = await api.get_disk_space();
      if (!diskData.success) {
        const reason = diskData.error?.trim() || 'The backend could not read disk usage.';
        logger.error('Disk space request was not successful', { error: reason });
        recordFailure(reason);
        return;
      }
      if (typeof diskData.percent !== 'number' || !Number.isFinite(diskData.percent) ||
        diskData.percent < 0 || diskData.percent > 100) {
        logger.error('Disk space response had no valid percent', { percent: diskData.percent });
        recordFailure('The backend returned an invalid disk usage value.');
        return;
      }
      setDiskSpacePercent(diskData.percent);
      setDiskSpaceError(null);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching disk space', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching disk space', { error: error.message });
      } else {
        logger.error('Unknown error fetching disk space', { error });
      }
      recordFailure(error instanceof Error ? error.message : 'Unknown error');
    }
  }, [recordFailure]);

  return {
    diskSpacePercent,
    diskSpaceError,
    fetchDiskSpace,
  };
}
