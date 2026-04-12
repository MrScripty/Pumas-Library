import { useCallback, useEffect, useMemo, useState } from 'react';
import { importAPI } from '../../api/import';
import { getLogger } from '../../utils/logger';
import {
  buildEmbeddedMetadataMatch,
  buildEntries,
  buildImportBatchSpecs,
  buildReviewFindings,
  buildShardedSetState,
  extractEmbeddedRepoId,
} from './modelImportWorkflowHelpers';
import type {
  DirectoryReviewFinding,
  ImportEntryStatus,
  ImportStep,
  ShardedSetInfo,
} from './modelImportWorkflowTypes';

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
  const [shardedSets, setShardedSets] = useState<ShardedSetInfo[]>([]);
  const [lookupProgress, setLookupProgress] = useState({ current: 0, total: 0 });
  const [expandedMetadata, setExpandedMetadata] = useState<Set<string>>(new Set());
  const [showEmbeddedMetadata, setShowEmbeddedMetadata] = useState<Set<string>>(new Set());
  const [showAllEmbeddedMetadata, setShowAllEmbeddedMetadata] = useState<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;

    const classifyPaths = async () => {
      setStep('classifying');
      setClassificationError(null);
      setEntries([]);
      setReviewFindings([]);
      setImportedCount(0);
      setFailedCount(0);
      setShardedSets([]);
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
  }, [importPaths]);

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
  }, []);

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

  useEffect(() => {
    if (fileEntries.length === 0) {
      setShardedSets([]);
      return;
    }

    const detectShards = async () => {
      try {
        const paths = fileEntries.map((entry) => entry.path);
        const result = await importAPI.detectShardedSets(paths);

        if (result.success && result.groups) {
          const { fileToSetMap, sets } = buildShardedSetState(result.groups);
          setShardedSets(sets);
          setEntries((prev) => {
            let changed = false;
            const next = prev.map((entry) => {
              const shardedSetKey =
                entry.kind === 'single_file' ? fileToSetMap[entry.path] : undefined;

              if (entry.shardedSetKey === shardedSetKey) {
                return entry;
              }

              changed = true;
              return {
                ...entry,
                shardedSetKey,
              };
            });

            return changed ? next : prev;
          });
        }
      } catch (error) {
        logger.error('Failed to detect sharded sets', { error });
      }
    };

    void detectShards();
  }, [fileEntries]);

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

  const toggleShardedSet = useCallback((key: string) => {
    setShardedSets((prev) => prev.map((set) => (
      set.key === key ? { ...set, expanded: !set.expanded } : set
    )));
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
                  embeddedMetadata: metadata ?? undefined,
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
      && entry.hfMetadata?.match_confidence === 1.0
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
