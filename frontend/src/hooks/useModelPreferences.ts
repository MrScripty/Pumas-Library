import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useModelPreferences');
const DEFAULT_APP_ID = 'comfyui';

type UseModelPreferencesOptions = {
  selectedAppId: string | null;
  defaultAppId?: string;
};

export function useModelPreferences({
  selectedAppId,
  defaultAppId = DEFAULT_APP_ID,
}: UseModelPreferencesOptions) {
  const [starredModels, setStarredModels] = useState<Set<string>>(new Set());
  const [excludedModels, setExcludedModels] = useState<Set<string>>(new Set());
  const exclusionRevisionRef = useRef(0);
  const activeAppId = selectedAppId ?? defaultAppId;

  useEffect(() => {
    let cancelled = false;
    const loadRevision = exclusionRevisionRef.current;

    if (!isAPIAvailable()) {
      return () => {
        cancelled = true;
      };
    }

    void api.get_link_exclusions(activeAppId).then((result) => {
      if (!cancelled && result.success && exclusionRevisionRef.current === loadRevision) {
        setExcludedModels(new Set(result.excluded_model_ids));
      }
    }).catch((error: unknown) => {
      logger.error('Failed to load link exclusions', { error });
    });

    return () => {
      cancelled = true;
    };
  }, [activeAppId]);

  const toggleStar = useCallback((modelId: string) => {
    setStarredModels((prev) => {
      const next = new Set(prev);
      if (next.has(modelId)) {
        next.delete(modelId);
      } else {
        next.add(modelId);
      }
      return next;
    });
  }, []);

  const toggleLink = useCallback((modelId: string) => {
    const wasExcluded = excludedModels.has(modelId);
    const nowExcluded = !wasExcluded;
    exclusionRevisionRef.current += 1;

    setExcludedModels((prev) => {
      const next = new Set(prev);
      if (nowExcluded) {
        next.add(modelId);
      } else {
        next.delete(modelId);
      }
      return next;
    });

    if (!isAPIAvailable()) {
      return;
    }

    void api.set_model_link_exclusion(modelId, activeAppId, nowExcluded).catch((error: unknown) => {
      logger.error('Failed to persist link exclusion', { modelId, error });
      setExcludedModels((prev) => {
        const next = new Set(prev);
        if (wasExcluded) {
          next.add(modelId);
        } else {
          next.delete(modelId);
        }
        return next;
      });
    });
  }, [activeAppId, excludedModels]);

  return {
    excludedModels,
    starredModels,
    toggleLink,
    toggleStar,
  };
}
