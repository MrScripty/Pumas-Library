import { useCallback, useEffect, useMemo, useState } from 'react';
import { importAPI } from '../../api/import';
import type {
  HFMetadataLookupResult,
  ModelImportSpec,
  SecurityTier,
} from '../../types/api';
import { getLogger } from '../../utils/logger';
import { getFilename, getSecurityTier } from './metadataUtils';

const logger = getLogger('ModelImportDialog');

export type ImportStep = 'review' | 'lookup' | 'importing' | 'complete';
export type MetadataStatus = 'pending' | 'found' | 'not_found' | 'error' | 'manual';

export interface FileImportStatus {
  path: string;
  filename: string;
  status: 'pending' | 'importing' | 'success' | 'error';
  error?: string;
  securityTier?: SecurityTier;
  securityAcknowledged?: boolean;
  hfMetadata?: HFMetadataLookupResult;
  metadataStatus?: MetadataStatus;
  shardedSetKey?: string;
  validFileType?: boolean;
  detectedFileType?: string;
  embeddedMetadata?: Record<string, unknown>;
  embeddedMetadataStatus?: 'pending' | 'loaded' | 'error' | 'unsupported';
}

export interface ShardedSetInfo {
  key: string;
  files: string[];
  complete: boolean;
  missingShards: number[];
  expanded: boolean;
}

interface UseModelImportWorkflowOptions {
  filePaths: string[];
  onImportComplete: () => void;
}

export function useModelImportWorkflow({
  filePaths,
  onImportComplete,
}: UseModelImportWorkflowOptions) {
  const [step, setStep] = useState<ImportStep>('review');
  const [files, setFiles] = useState<FileImportStatus[]>([]);
  const [importedCount, setImportedCount] = useState(0);
  const [failedCount, setFailedCount] = useState(0);
  const [shardedSets, setShardedSets] = useState<ShardedSetInfo[]>([]);
  const [lookupProgress, setLookupProgress] = useState({ current: 0, total: 0 });
  const [expandedMetadata, setExpandedMetadata] = useState<Set<string>>(new Set());
  const [showEmbeddedMetadata, setShowEmbeddedMetadata] = useState<Set<string>>(new Set());
  const [showAllEmbeddedMetadata, setShowAllEmbeddedMetadata] = useState<Set<string>>(new Set());

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
    setFiles((prev) => {
      const file = prev.find((entry) => entry.path === path);
      if (!file) return prev;
      if (!file.embeddedMetadata
        && file.embeddedMetadataStatus !== 'error'
        && file.embeddedMetadataStatus !== 'unsupported'
        && file.embeddedMetadataStatus !== 'pending') {
        needsLoad = true;
        return prev.map((entry) => (
          entry.path === path ? { ...entry, embeddedMetadataStatus: 'pending' } : entry
        ));
      }
      return prev;
    });

    setShowEmbeddedMetadata((prev) => {
      const isCurrentlyShowingEmbedded = prev.has(path);

      if (!isCurrentlyShowingEmbedded && needsLoad) {
        importAPI.getEmbeddedMetadata(path).then((result) => {
          setFiles((prevFiles) => prevFiles.map((entry) => {
            if (entry.path !== path) return entry;
            if (result.success && result.metadata) {
              return {
                ...entry,
                embeddedMetadata: result.metadata,
                embeddedMetadataStatus: 'loaded',
              };
            }
            if (result.file_type === 'unsupported') {
              return { ...entry, embeddedMetadataStatus: 'unsupported' };
            }
            return { ...entry, embeddedMetadataStatus: 'error' };
          }));
        }).catch((error: unknown) => {
          logger.error('Failed to fetch embedded metadata', { path, error: String(error) });
          setFiles((prevFiles) => prevFiles.map((entry) => (
            entry.path === path ? { ...entry, embeddedMetadataStatus: 'error' } : entry
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

  useEffect(() => {
    const fileStatuses: FileImportStatus[] = filePaths.map((path) => {
      const filename = getFilename(path);
      const securityTier = getSecurityTier(filename);
      return {
        path,
        filename,
        status: 'pending',
        securityTier,
        securityAcknowledged: securityTier !== 'pickle',
        metadataStatus: 'pending',
      };
    });
    setFiles(fileStatuses);
  }, [filePaths]);

  useEffect(() => {
    if (files.length === 0) return;

    const detectShards = async () => {
      try {
        const paths = files.map((file) => file.path);
        const result = await importAPI.detectShardedSets(paths);

        if (result.success && result.groups) {
          const sets: ShardedSetInfo[] = [];
          const fileToSetMap: Record<string, string> = {};

          Object.entries(result.groups).forEach(([key, group]) => {
            if (group.files.length > 1) {
              sets.push({
                key,
                files: group.files,
                complete: group.validation.complete,
                missingShards: group.validation.missing_shards,
                expanded: false,
              });
              group.files.forEach((file) => {
                fileToSetMap[file] = key;
              });
            }
          });

          setShardedSets(sets);
          setFiles((prev) => prev.map((file) => ({
            ...file,
            shardedSetKey: fileToSetMap[file.path],
          })));
        }
      } catch (error) {
        logger.error('Failed to detect sharded sets', { error });
      }
    };

    void detectShards();
  }, [files.length]);

  const allPickleAcknowledged = files.every(
    (file) => file.securityTier !== 'pickle' || file.securityAcknowledged
  );

  const toggleSecurityAck = useCallback((index: number) => {
    setFiles((prev) => {
      const file = prev[index];
      if (!file) return prev;
      const updated = [...prev];
      updated[index] = {
        ...file,
        securityAcknowledged: !file.securityAcknowledged,
      };
      return updated;
    });
  }, []);

  const removeFile = useCallback((index: number) => {
    setFiles((prev) => prev.filter((_, currentIndex) => currentIndex !== index));
  }, []);

  const toggleShardedSet = useCallback((key: string) => {
    setShardedSets((prev) => prev.map((set) => (
      set.key === key ? { ...set, expanded: !set.expanded } : set
    )));
  }, []);

  const performMetadataLookup = useCallback(async () => {
    setStep('lookup');
    const totalFiles = files.length;
    setLookupProgress({ current: 0, total: totalFiles });

    const filesToProcess = files.map((file) => ({ path: file.path, filename: file.filename }));

    for (let index = 0; index < filesToProcess.length; index += 1) {
      const file = filesToProcess[index];
      if (!file) continue;

      setLookupProgress({ current: index + 1, total: totalFiles });

      try {
        const typeResult = await importAPI.validateFileType(file.path);

        setFiles((prev) => prev.map((entry) => {
          if (entry.path !== file.path) return entry;
          return {
            ...entry,
            validFileType: typeResult.valid,
            detectedFileType: typeResult.detected_type,
            metadataStatus: typeResult.valid ? entry.metadataStatus : 'error',
          };
        }));

        if (!typeResult.valid) {
          continue;
        }

        let skipHfSearch = false;
        let embeddedRepoId: string | null = null;

        if (typeResult.detected_type === 'gguf' || typeResult.detected_type === 'safetensors') {
          try {
            const embeddedResult = await importAPI.getEmbeddedMetadata(file.path);

            if (embeddedResult.success && embeddedResult.metadata) {
              const metadata = embeddedResult.metadata;
              setFiles((prev) => prev.map((entry) => {
                if (entry.path !== file.path) return entry;
                return {
                  ...entry,
                  embeddedMetadata: metadata ?? undefined,
                  embeddedMetadataStatus: 'loaded',
                };
              }));

              const repoUrl = embeddedResult.metadata['general.repo_url'];
              if (repoUrl && typeof repoUrl === 'string') {
                const match = repoUrl.match(/huggingface\.co\/([^/]+\/[^/]+)/);
                if (match && match[1]) {
                  embeddedRepoId = match[1];
                  skipHfSearch = true;
                }
              }

              if (!skipHfSearch) {
                const quantizedBy = embeddedResult.metadata['general.quantized_by'];
                const name = embeddedResult.metadata['general.name'];
                if (quantizedBy && name) {
                  embeddedRepoId = `${String(quantizedBy)}/${String(name)}`;
                  skipHfSearch = true;
                }
              }
            }
          } catch (error) {
            logger.debug('Failed to extract embedded metadata early', { error });
          }
        }

        if (skipHfSearch && embeddedRepoId) {
          setFiles((prev) => prev.map((entry) => {
            if (entry.path !== file.path) return entry;
            return {
              ...entry,
              hfMetadata: {
                repo_id: embeddedRepoId,
                official_name: file.filename,
                family: '',
                match_method: 'filename_exact',
                match_confidence: 0.9,
                requires_confirmation: false,
              },
              metadataStatus: 'found',
            };
          }));
          continue;
        }

        const result = await importAPI.lookupHFMetadata(file.filename, file.path);
        setFiles((prev) => prev.map((entry) => {
          if (entry.path !== file.path) return entry;
          if (result.success && result.found && result.metadata) {
            return {
              ...entry,
              hfMetadata: result.metadata,
              metadataStatus: 'found',
            };
          }
          return {
            ...entry,
            metadataStatus: 'not_found',
          };
        }));
      } catch (error) {
        logger.error('Metadata lookup failed', { file: file.filename, error });
        setFiles((prev) => prev.map((entry) => (
          entry.path === file.path ? { ...entry, metadataStatus: 'error' } : entry
        )));
      }
    }
  }, [files]);

  const startImport = useCallback(async () => {
    if (!allPickleAcknowledged || files.length === 0) return;

    setStep('importing');
    const specs: ModelImportSpec[] = files
      .filter((file) => file.validFileType !== false)
      .map((file) => ({
        path: file.path,
        family: file.hfMetadata?.family || 'imported',
        official_name: file.hfMetadata?.official_name || file.filename.replace(/\.[^.]+$/, ''),
        repo_id: file.hfMetadata?.repo_id,
        model_type: file.hfMetadata?.model_type,
        subtype: file.hfMetadata?.subtype,
        tags: file.hfMetadata?.tags,
        security_acknowledged: file.securityAcknowledged,
      }));

    try {
      setFiles((prev) => prev.map((file) => ({
        ...file,
        status: file.validFileType !== false ? 'importing' : 'error',
      })));

      const result = await importAPI.importBatch(specs);
      setFiles((prev) => prev.map((file) => {
        if (file.validFileType === false) {
          return {
            ...file,
            status: 'error',
            error: `Invalid file type: ${file.detectedFileType}`,
          };
        }
        const importResult = result.results.find((entry) => entry.path === file.path);
        if (importResult) {
          return {
            ...file,
            status: importResult.success ? 'success' : 'error',
            error: importResult.error,
            securityTier: importResult.security_tier || file.securityTier,
          };
        }
        return file;
      }));

      setImportedCount(result.imported);
      setFailedCount(result.failed + files.filter((file) => file.validFileType === false).length);
      setStep('complete');

      if (result.imported > 0) {
        onImportComplete();
      }
    } catch (error) {
      logger.error('Import batch failed', { error });
      setFiles((prev) => prev.map((file) => ({
        ...file,
        status: 'error',
        error: error instanceof Error ? error.message : 'Import failed',
      })));
      setFailedCount(files.length);
      setStep('complete');
    }
  }, [allPickleAcknowledged, files, onImportComplete]);

  const proceedToLookup = useCallback(() => {
    void performMetadataLookup();
  }, [performMetadataLookup]);

  const pickleFilesCount = files.filter((file) => file.securityTier === 'pickle').length;
  const acknowledgedCount = files.filter(
    (file) => file.securityTier === 'pickle' && file.securityAcknowledged
  ).length;
  const invalidFileCount = files.filter((file) => file.validFileType === false).length;
  const verifiedCount = files.filter(
    (file) => file.hfMetadata?.match_method === 'hash' && file.hfMetadata?.match_confidence === 1.0
  ).length;
  const standaloneFiles = useMemo(
    () => files.filter((file) => !file.shardedSetKey),
    [files]
  );

  return {
    step,
    files,
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
    removeFile,
    toggleShardedSet,
    proceedToLookup,
    startImport,
    pickleFilesCount,
    acknowledgedCount,
    invalidFileCount,
    verifiedCount,
    standaloneFiles,
  };
}
