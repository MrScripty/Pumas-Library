import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import { importAPI } from '../../api/import';
import { getLogger } from '../../utils/logger';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';

const logger = getLogger('ModelImportDialog');

interface UseEmbeddedMetadataTogglesOptions {
  setEntries: Dispatch<SetStateAction<ImportEntryStatus[]>>;
}

export function useEmbeddedMetadataToggles({
  setEntries,
}: UseEmbeddedMetadataTogglesOptions) {
  const [showEmbeddedMetadata, setShowEmbeddedMetadata] = useState<Set<string>>(new Set());
  const [showAllEmbeddedMetadata, setShowAllEmbeddedMetadata] = useState<Set<string>>(new Set());

  const toggleMetadataSource = useCallback(async (path: string) => {
    let needsLoad = false;
    setEntries((prev) => {
      const entry = prev.find((candidate) => candidate.path === path);
      if (!entry || entry.kind !== 'single_file') return prev;
      if (!entry.embeddedMetadata
        && entry.embeddedMetadataStatus !== 'error'
        && entry.embeddedMetadataStatus !== 'unsupported'
        && entry.embeddedMetadataStatus !== 'pending') {
        needsLoad = true;
        return prev.map((candidate) => (
          candidate.path === path
            ? { ...candidate, embeddedMetadataStatus: 'pending' }
            : candidate
        ));
      }
      return prev;
    });

    setShowEmbeddedMetadata((prev) => {
      const isCurrentlyShowingEmbedded = prev.has(path);

      if (!isCurrentlyShowingEmbedded && needsLoad) {
        importAPI.getEmbeddedMetadata(path).then((result) => {
          setEntries((prevEntries) => prevEntries.map((candidate) => {
            if (candidate.path !== path) return candidate;
            if (result.success && result.metadata) {
              return {
                ...candidate,
                embeddedMetadata: result.metadata,
                embeddedMetadataStatus: 'loaded',
              };
            }
            if (result.file_type === 'unsupported') {
              return { ...candidate, embeddedMetadataStatus: 'unsupported' };
            }
            return { ...candidate, embeddedMetadataStatus: 'error' };
          }));
        }).catch((error: unknown) => {
          logger.error('Failed to fetch embedded metadata', { path, error: String(error) });
          setEntries((prevEntries) => prevEntries.map((candidate) => (
            candidate.path === path
              ? { ...candidate, embeddedMetadataStatus: 'error' }
              : candidate
          )));
        });
      }

      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, [setEntries]);

  const toggleShowAllEmbeddedMetadata = useCallback((path: string) => {
    setShowAllEmbeddedMetadata((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  return {
    showEmbeddedMetadata,
    showAllEmbeddedMetadata,
    toggleMetadataSource,
    toggleShowAllEmbeddedMetadata,
  };
}
