/**
 * Disk space monitoring hook
 *
 * Polls disk space usage periodically.
 */

import { useState, useCallback } from 'react';
import { pywebview } from '../api/pywebview';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useDiskSpace');

export function useDiskSpace() {
  const [diskSpacePercent, setDiskSpacePercent] = useState(0);

  const fetchDiskSpace = useCallback(async () => {
    try {
      if (pywebview.isAvailable()) {
        const diskData = await pywebview.getDiskSpace();
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
