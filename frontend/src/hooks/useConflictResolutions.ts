import { useCallback, useMemo, useState } from 'react';
import type { MappingAction } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('useConflictResolutions');

export type ConflictResolutionAction = 'skip' | 'overwrite' | 'rename';
export type ConflictResolutions = Record<string, ConflictResolutionAction>;

interface UseConflictResolutionsOptions {
  conflicts: MappingAction[];
  onApply: (resolutions: ConflictResolutions) => Promise<void>;
}

export function useConflictResolutions({
  conflicts,
  onApply,
}: UseConflictResolutionsOptions) {
  const [resolutions, setResolutions] = useState<ConflictResolutions>({});
  const [isApplying, setIsApplying] = useState(false);
  const [expandedConflict, setExpandedConflict] = useState<string | null>(null);

  const effectiveResolutions = useMemo(() => {
    const result: ConflictResolutions = {};
    for (const conflict of conflicts) {
      result[conflict.model_id] = resolutions[conflict.model_id] || 'skip';
    }
    return result;
  }, [conflicts, resolutions]);

  const resolutionCounts = useMemo(() => {
    const counts = { skip: 0, overwrite: 0, rename: 0 };
    for (const resolution of Object.values(effectiveResolutions)) {
      counts[resolution]++;
    }
    return counts;
  }, [effectiveResolutions]);

  const handleResolutionChange = useCallback(
    (modelId: string, action: ConflictResolutionAction) => {
      setResolutions((prev) => ({
        ...prev,
        [modelId]: action,
      }));
    },
    []
  );

  const handleApplyToAll = useCallback((action: ConflictResolutionAction) => {
    const newResolutions: ConflictResolutions = {};
    for (const conflict of conflicts) {
      newResolutions[conflict.model_id] = action;
    }
    setResolutions(newResolutions);
  }, [conflicts]);

  const handleApply = useCallback(async () => {
    setIsApplying(true);
    try {
      await onApply(effectiveResolutions);
      logger.info('Applied conflict resolutions', { resolutions: effectiveResolutions });
    } catch (error) {
      logger.error('Failed to apply resolutions', { error });
    } finally {
      setIsApplying(false);
    }
  }, [effectiveResolutions, onApply]);

  const toggleExpanded = useCallback((modelId: string) => {
    setExpandedConflict((prev) => (prev === modelId ? null : modelId));
  }, []);

  return {
    effectiveResolutions,
    expandedConflict,
    isApplying,
    resolutionCounts,
    handleApply,
    handleApplyToAll,
    handleResolutionChange,
    toggleExpanded,
  };
}
