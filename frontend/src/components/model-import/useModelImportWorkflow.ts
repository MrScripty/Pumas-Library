import { useCallback, useEffect, useMemo, useState } from 'react';
import { importAPI } from '../../api/import';
import { getLogger } from '../../utils/logger';
import {
  buildEntries,
  buildImportBatchSpecs,
  buildReviewFindings,
} from './modelImportWorkflowHelpers';
import { runMetadataLookup } from './modelImportMetadataLookup';
import type {
  DirectoryReviewFinding,
  ImportEntryStatus,
  ImportStep,
} from './modelImportWorkflowTypes';
import { useEmbeddedMetadataToggles } from './useEmbeddedMetadataToggles';
import { useShardedSetDetection } from './useShardedSetDetection';

export type {
  DirectoryReviewFinding,
  ImportEntryKind,
  ImportEntryStatus,
  ImportStep,
  MetadataStatus,
  ShardedSetInfo,
} from './modelImportWorkflowTypes';

const logger = getLogger('ModelImportDialog');

interface UseModelImportWorkflowOptions {
  importPaths: string[];
  onImportComplete: () => void;
}

export function useModelImportWorkflow({
  importPaths,
  onImportComplete,
}: UseModelImportWorkflowOptions) {
  const [step, setStep] = useState<ImportStep>('classifying');
  const [entries, setEntries] = useState<ImportEntryStatus[]>([]);
  const [reviewFindings, setReviewFindings] = useState<DirectoryReviewFinding[]>([]);
  const [classificationError, setClassificationError] = useState<string | null>(null);
  const [importedCount, setImportedCount] = useState(0);
  const [failedCount, setFailedCount] = useState(0);
  const [lookupProgress, setLookupProgress] = useState({ current: 0, total: 0 });
  const [expandedMetadata, setExpandedMetadata] = useState<Set<string>>(new Set());
  const {
    showEmbeddedMetadata,
    showAllEmbeddedMetadata,
    toggleMetadataSource,
    toggleShowAllEmbeddedMetadata,
  } = useEmbeddedMetadataToggles({ setEntries });

  const fileEntries = useMemo(
    () => entries.filter((entry) => entry.kind === 'single_file'),
    [entries]
  );

  const lookupEntries = useMemo(
    () => entries.filter(
      (entry) => entry.kind === 'single_file' || entry.kind === 'external_diffusers_bundle'
    ),
    [entries]
  );

  const nonFileEntries = useMemo(
    () => entries.filter((entry) => entry.kind !== 'single_file'),
    [entries]
  );

  const {
    shardedSets,
    clearShardedSets,
    toggleShardedSet,
  } = useShardedSetDetection({ fileEntries, setEntries });

  useEffect(() => {
    let cancelled = false;

    const classifyPaths = async () => {
      setStep('classifying');
      setClassificationError(null);
      setEntries([]);
      setReviewFindings([]);
      setImportedCount(0);
      setFailedCount(0);
      clearShardedSets();
      setLookupProgress({ current: 0, total: 0 });

      if (importPaths.length === 0) {
        setStep('review');
        return;
      }

      try {
        const results = await importAPI.classifyImportPaths(importPaths);
        if (cancelled) return;
        setEntries(buildEntries(results));
        setReviewFindings(buildReviewFindings(results));
      } catch (error) {
        if (cancelled) return;
        logger.error('Failed to classify import paths', { error });
        setClassificationError(
          error instanceof Error ? error.message : 'Failed to classify import paths'
        );
      } finally {
        if (!cancelled) {
          setStep('review');
        }
      }
    };

    void classifyPaths();

    return () => {
      cancelled = true;
    };
  }, [clearShardedSets, importPaths]);

  const toggleMetadataExpand = useCallback((path: string) => {
    setExpandedMetadata((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const allPickleAcknowledged = entries.every(
    (entry) => entry.securityTier !== 'pickle' || entry.securityAcknowledged
  );

  const toggleSecurityAck = useCallback((path: string) => {
    setEntries((prev) => prev.map((entry) => (
      entry.path === path
        ? { ...entry, securityAcknowledged: !entry.securityAcknowledged }
        : entry
    )));
  }, []);

  const removeEntry = useCallback((path: string) => {
    setEntries((prev) => prev.filter((entry) => entry.path !== path));
  }, []);

  const performMetadataLookup = useCallback(async () => {
    const entriesToProcess = lookupEntries.map((entry) => ({
      path: entry.path,
      filename: entry.filename,
      kind: entry.kind,
    }));
    setStep('lookup');
    setLookupProgress({ current: 0, total: entriesToProcess.length });

    if (entriesToProcess.length === 0) {
      return;
    }

    await runMetadataLookup({
      entriesToProcess,
      setEntries,
      setLookupProgress,
    });
  }, [lookupEntries]);

  const startImport = useCallback(async () => {
    if (!allPickleAcknowledged || entries.length === 0) return;

    setStep('importing');
    const invalidFileEntries = fileEntries.filter((entry) => entry.validFileType === false);
    const batchEntries = entries.filter(
      (entry) => !(entry.kind === 'single_file' && entry.validFileType === false)
    );

    const batchSpecs = buildImportBatchSpecs(batchEntries);

    try {
      setEntries((prev) => prev.map((entry) => {
        if (entry.kind === 'single_file' && entry.validFileType === false) {
          return {
            ...entry,
            status: 'error',
            error: `Invalid file type: ${entry.detectedFileType}`,
          };
        }

        return { ...entry, status: 'importing' };
      }));

      let imported = 0;
      let failed = invalidFileEntries.length;

      if (batchSpecs.length > 0) {
        const result = await importAPI.importBatch(batchSpecs);
        imported += result.imported;
        failed += result.failed;

        setEntries((prev) => prev.map((entry) => {
          const importResult = result.results.find((candidate) => candidate.path === entry.path);
          if (!importResult) {
            return entry;
          }

          return {
            ...entry,
            status: importResult.success ? 'success' : 'error',
            error: importResult.error,
            securityTier: importResult.security_tier || entry.securityTier,
          };
        }));
      }

      setImportedCount(imported);
      setFailedCount(failed);
      setStep('complete');

      if (imported > 0) {
        onImportComplete();
      }
    } catch (error) {
      logger.error('Import batch failed', { error });
      setEntries((prev) => prev.map((entry) => ({
        ...entry,
        status: 'error',
        error: error instanceof Error ? error.message : 'Import failed',
      })));
      setFailedCount(entries.length);
      setStep('complete');
    }
  }, [allPickleAcknowledged, entries, fileEntries, onImportComplete]);

  const proceedToLookup = useCallback(() => {
    if (lookupEntries.length === 0) {
      void startImport();
      return;
    }
    void performMetadataLookup();
  }, [lookupEntries.length, performMetadataLookup, startImport]);

  const pickleFilesCount = entries.filter((entry) => entry.securityTier === 'pickle').length;
  const acknowledgedCount = entries.filter(
    (entry) => entry.securityTier === 'pickle' && entry.securityAcknowledged
  ).length;
  const invalidFileCount = fileEntries.filter((entry) => entry.validFileType === false).length;
  const verifiedCount = fileEntries.filter(
    (entry) => entry.hfMetadata?.match_method === 'hash'
      && entry.hfMetadata.match_confidence === 1.0
  ).length;
  const standaloneEntries = useMemo(
    () => entries.filter((entry) => entry.kind !== 'single_file' || !entry.shardedSetKey),
    [entries]
  );
  const blockedFindings = useMemo(
    () => reviewFindings.filter((finding) => finding.kind !== 'multi_model_container'),
    [reviewFindings]
  );
  const containerFindings = useMemo(
    () => reviewFindings.filter((finding) => finding.kind === 'multi_model_container'),
    [reviewFindings]
  );

  return {
    step,
    entries,
    fileEntries,
    nonFileEntries,
    reviewFindings,
    blockedFindings,
    containerFindings,
    classificationError,
    importedCount,
    failedCount,
    shardedSets,
    lookupProgress,
    expandedMetadata,
    showEmbeddedMetadata,
    showAllEmbeddedMetadata,
    allPickleAcknowledged,
    toggleMetadataExpand,
    toggleMetadataSource,
    toggleShowAllEmbeddedMetadata,
    toggleSecurityAck,
    removeEntry,
    toggleShardedSet,
    proceedToLookup,
    startImport,
    pickleFilesCount,
    acknowledgedCount,
    invalidFileCount,
    verifiedCount,
    standaloneEntries,
  };
}
