import type { Dispatch, SetStateAction } from 'react';
import { importAPI } from '../../api/import';
import { getLogger } from '../../utils/logger';
import {
  buildEmbeddedMetadataMatch,
  extractEmbeddedRepoId,
} from './modelImportWorkflowHelpers';
import type { ImportEntryKind, ImportEntryStatus } from './modelImportWorkflowTypes';

const logger = getLogger('ModelImportDialog');

export interface MetadataLookupEntry {
  filename: string;
  kind: ImportEntryKind;
  path: string;
}

interface RunMetadataLookupOptions {
  entriesToProcess: MetadataLookupEntry[];
  setEntries: Dispatch<SetStateAction<ImportEntryStatus[]>>;
  setLookupProgress: Dispatch<SetStateAction<{ current: number; total: number }>>;
}

export async function runMetadataLookup({
  entriesToProcess,
  setEntries,
  setLookupProgress,
}: RunMetadataLookupOptions) {
  for (let index = 0; index < entriesToProcess.length; index += 1) {
    const entry = entriesToProcess[index];
    if (!entry) continue;

    try {
      if (entry.kind === 'external_diffusers_bundle') {
        const result = await importAPI.lookupHFMetadataForBundleDirectory(entry.path);
        setEntries((prev) => prev.map((candidate) => {
          if (candidate.path !== entry.path) return candidate;
          if (result.success && result.found && result.metadata) {
            return {
              ...candidate,
              hfMetadata: result.metadata,
              metadataStatus: 'found',
            };
          }
          return {
            ...candidate,
            metadataStatus: result.success ? 'not_found' : 'error',
          };
        }));
        setLookupProgress({ current: index + 1, total: entriesToProcess.length });
        continue;
      }

      const typeResult = await importAPI.validateFileType(entry.path);

      setEntries((prev) => prev.map((candidate) => {
        if (candidate.path !== entry.path) return candidate;
        return {
          ...candidate,
          validFileType: typeResult.valid,
          detectedFileType: typeResult.detected_type,
          metadataStatus: typeResult.valid ? candidate.metadataStatus : 'error',
        };
      }));

      if (!typeResult.valid) {
        setLookupProgress({ current: index + 1, total: entriesToProcess.length });
        continue;
      }

      let skipHfSearch = false;
      let embeddedRepoId: string | null = null;

      if (typeResult.detected_type === 'gguf' || typeResult.detected_type === 'safetensors') {
        try {
          const embeddedResult = await importAPI.getEmbeddedMetadata(entry.path);

          if (embeddedResult.success && embeddedResult.metadata) {
            const metadata = embeddedResult.metadata;
            setEntries((prev) => prev.map((candidate) => {
              if (candidate.path !== entry.path) return candidate;
              return {
                ...candidate,
                embeddedMetadata: metadata,
                embeddedMetadataStatus: 'loaded',
              };
            }));
            embeddedRepoId = extractEmbeddedRepoId(metadata);
            if (embeddedRepoId) {
              skipHfSearch = true;
            }
          }
        } catch (error) {
          logger.debug('Failed to extract embedded metadata early', { error });
        }
      }

      if (skipHfSearch && embeddedRepoId) {
        setEntries((prev) => prev.map((candidate) => {
          if (candidate.path !== entry.path) return candidate;
          return {
            ...candidate,
            hfMetadata: buildEmbeddedMetadataMatch(candidate, embeddedRepoId),
            metadataStatus: 'found',
          };
        }));
        setLookupProgress({ current: index + 1, total: entriesToProcess.length });
        continue;
      }

      const result = await importAPI.lookupHFMetadata(entry.filename, entry.path);
      setEntries((prev) => prev.map((candidate) => {
        if (candidate.path !== entry.path) return candidate;
        if (result.success && result.found && result.metadata) {
          return {
            ...candidate,
            hfMetadata: result.metadata,
            metadataStatus: 'found',
          };
        }
        return {
          ...candidate,
          metadataStatus: 'not_found',
        };
      }));
      setLookupProgress({ current: index + 1, total: entriesToProcess.length });
    } catch (error) {
      logger.error('Metadata lookup failed', { file: entry.filename, error });
      setEntries((prev) => prev.map((candidate) => (
        candidate.path === entry.path
          ? { ...candidate, metadataStatus: 'error' }
          : candidate
      )));
      setLookupProgress({ current: index + 1, total: entriesToProcess.length });
    }
  }
}
