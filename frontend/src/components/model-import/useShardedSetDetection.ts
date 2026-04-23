import { useCallback, useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { importAPI } from '../../api/import';
import { getLogger } from '../../utils/logger';
import { buildShardedSetState } from './modelImportWorkflowHelpers';
import type { ImportEntryStatus, ShardedSetInfo } from './modelImportWorkflowTypes';

const logger = getLogger('ModelImportDialog');

interface UseShardedSetDetectionOptions {
  fileEntries: ImportEntryStatus[];
  setEntries: Dispatch<SetStateAction<ImportEntryStatus[]>>;
}

export function useShardedSetDetection({
  fileEntries,
  setEntries,
}: UseShardedSetDetectionOptions) {
  const [shardedSets, setShardedSets] = useState<ShardedSetInfo[]>([]);

  useEffect(() => {
    if (fileEntries.length === 0) {
      setShardedSets([]);
      return;
    }

    const detectShards = async () => {
      try {
        const paths = fileEntries.map((entry) => entry.path);
        const result = await importAPI.detectShardedSets(paths);

        if (result.success) {
          const { fileToSetMap, sets } = buildShardedSetState(result.groups);
          setShardedSets(sets);
          setEntries((prev) => {
            const next = prev.map((entry) => {
              const shardedSetKey =
                entry.kind === 'single_file' ? fileToSetMap[entry.path] : undefined;

              if (entry.shardedSetKey === shardedSetKey) {
                return entry;
              }

              return {
                ...entry,
                shardedSetKey,
              };
            });

            return next.some((entry, index) => entry !== prev[index]) ? next : prev;
          });
        }
      } catch (error) {
        logger.error('Failed to detect sharded sets', { error });
      }
    };

    void detectShards();
  }, [fileEntries, setEntries]);

  const clearShardedSets = useCallback(() => {
    setShardedSets([]);
  }, []);

  const toggleShardedSet = useCallback((key: string) => {
    setShardedSets((prev) => prev.map((set) => (
      set.key === key ? { ...set, expanded: !set.expanded } : set
    )));
  }, []);

  return {
    shardedSets,
    clearShardedSets,
    toggleShardedSet,
  };
}
