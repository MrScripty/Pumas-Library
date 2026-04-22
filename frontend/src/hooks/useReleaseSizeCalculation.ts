import { useEffect, useRef } from 'react';
import { api } from '../api/adapter';
import { APIError } from '../errors';
import type { VersionRelease } from './useVersions';
import { getLogger } from '../utils/logger';

const logger = getLogger('useReleaseSizeCalculation');

interface UseReleaseSizeCalculationOptions {
  appId?: string;
  availableVersions: VersionRelease[];
  isOpen: boolean;
  onRefreshAll: (forceRefresh?: boolean) => Promise<void>;
}

export function useReleaseSizeCalculation({
  appId,
  availableVersions,
  isOpen,
  onRefreshAll,
}: UseReleaseSizeCalculationOptions) {
  const sizeCalcTriggeredRef = useRef(false);

  useEffect(() => {
    if (!isOpen) {
      sizeCalcTriggeredRef.current = false;
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen || availableVersions.length === 0 || sizeCalcTriggeredRef.current) {
      return;
    }

    const releasesNeedingSize = availableVersions.filter(
      (release) =>
        release.tagName &&
        (release.totalSize === null || release.totalSize === undefined)
    );

    if (releasesNeedingSize.length === 0) {
      return;
    }

    sizeCalcTriggeredRef.current = true;

    const calculateSizes = async () => {
      logger.info('Starting background size calculation', {
        releaseCount: releasesNeedingSize.length,
      });

      for (const release of releasesNeedingSize) {
        try {
          await api.calculate_release_size(release.tagName, false, appId);
        } catch (error) {
          if (error instanceof APIError) {
            logger.error('API error calculating release size', {
              error: error.message,
              endpoint: error.endpoint,
              tag: release.tagName,
            });
          } else if (error instanceof Error) {
            logger.error('Failed to calculate release size', {
              error: error.message,
              tag: release.tagName,
            });
          } else {
            logger.error('Unknown error calculating release size', {
              error,
              tag: release.tagName,
            });
          }
        }
      }

      logger.info('Size calculation complete, refreshing versions');
      await onRefreshAll(false);
    };

    calculateSizes().catch((error: unknown) => {
      if (error instanceof APIError) {
        logger.error('API error during background size calculation', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof Error) {
        logger.error('Error during background size calculation', {
          error: error.message,
        });
      } else {
        logger.error('Unknown error during background size calculation', {
          error: String(error),
        });
      }
    });
  }, [appId, isOpen, availableVersions, onRefreshAll]);
}
