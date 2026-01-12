/**
 * Disk space monitoring hook
 *
 * Polls disk space usage periodically.
 */

import { useState, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useDiskSpace');

export function useDiskSpace() {
  const [diskSpacePercent, setDiskSpacePercent] = useState(0);

  const fetchDiskSpace = useCallback(async () => {
    try {
      if (isAPIAvailable()) {
        const diskData = await api.get_disk_space();
        if (diskData.success) {
          setDiskSpacePercent(diskData.percent || 0);
        }
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching disk space', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching disk space', { error: error.message });
      } else {
        logger.error('Unknown error fetching disk space', { error });
      }
    }
  }, []);

  return {
    diskSpacePercent,
    fetchDiskSpace,
  };
}
