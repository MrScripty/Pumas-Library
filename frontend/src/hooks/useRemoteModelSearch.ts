/**
 * Remote Model Search Hook
 *
 * Handles searching Hugging Face for models with debouncing.
 * Extracted from ModelManager.tsx
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { RemoteModelInfo, RemoteSearchSource } from '../types/apps';
import { effectiveSearchSource, presentSearchResults } from '../utils/hfCachedDiscovery';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useRemoteModelSearch');
const DEFAULT_HYDRATE_LIMIT = 6;
const EMPTY_RESULTS: RemoteModelInfo[] = [];

interface UseRemoteModelSearchOptions {
  enabled: boolean;
  searchQuery: string;
  debounceMs?: number;
  source?: RemoteSearchSource;
}

function hasExactDownloadDetails(model: RemoteModelInfo): boolean {
  if (typeof model.totalSizeBytes === 'number' && model.totalSizeBytes > 0) {
    return true;
  }

  return (
    model.downloadOptions?.some(
      (option) =>
        (typeof option.sizeBytes === 'number' && option.sizeBytes > 0) || Boolean(option.fileGroup)
    ) ?? false
  );
}

export function useRemoteModelSearch({
  enabled,
  searchQuery,
  debounceMs = 300,
  source = 'huggingface',
}: UseRemoteModelSearchOptions) {
  const [results, setResults] = useState<RemoteModelInfo[]>([]);
  const [resultContext, setResultContext] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [hydratingRepoIds, setHydratingRepoIds] = useState<Set<string>>(new Set());
  const generationRef = useRef(0);
  const resultsRef = useRef<RemoteModelInfo[]>([]);
  const inFlightHydrationsRef = useRef<Map<string, Promise<void>>>(new Map());
  const selectedSource = effectiveSearchSource(searchQuery, source);
  const requestContext = JSON.stringify([enabled, selectedSource, searchQuery.trim()]);
  const visibleResults = resultContext === requestContext ? results : EMPTY_RESULTS;

  useEffect(() => () => {
    generationRef.current += 1;
    inFlightHydrationsRef.current.clear();
  }, []);

  useEffect(() => {
    resultsRef.current = visibleResults;
  }, [visibleResults]);

  // Get unique kinds from results
  const kinds = useMemo(() => {
    const kindSet = new Set<string>();
    visibleResults.forEach((model) => {
      if (model.kind && model.kind !== 'unknown') {
        kindSet.add(model.kind);
      }
    });
    return ['all', ...Array.from(kindSet).sort()];
  }, [visibleResults]);

  useEffect(() => {
    generationRef.current += 1;
    inFlightHydrationsRef.current.clear();
    setHydratingRepoIds(new Set());
    setResults([]);
    setError(null);
    setIsLoading(false);

    if (!enabled) {
      return;
    }

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery && selectedSource !== 'cached') {
      setResults([]);
      setError(null);
      setIsLoading(false);
      return;
    }

    let isActive = true;
    const generation = generationRef.current;
    const handle = setTimeout(async () => {
      if (!isAPIAvailable()) {
        if (isActive) {
          setError('Hugging Face search is unavailable.');
          setResults([]);
          setIsLoading(false);
        }
        return;
      }

      setIsLoading(true);
      setError(null);
      try {
        const query = selectedSource === 'cached' && !trimmedQuery.startsWith('cache:')
          ? `cache:${trimmedQuery}` : trimmedQuery;
        const result = await api.search_hf_models(query, null, 25,
          selectedSource === 'cached' ? 0 : DEFAULT_HYDRATE_LIMIT);
        if (!isActive || generation !== generationRef.current) {
          return;
        }
        if (result.success) {
          setResults(presentSearchResults(result.models as RemoteModelInfo[], selectedSource));
          setResultContext(requestContext);
        } else {
          setError(result.error || 'Search failed.');
          setResults([]);
        }
      } catch (error) {
        if (!isActive) {
          return;
        }
        if (error instanceof APIError) {
          logger.error('API error searching Hugging Face models', { error: error.message, endpoint: error.endpoint, query: trimmedQuery });
          setError(error.message);
        } else if (error instanceof Error) {
          logger.error('Unexpected error searching Hugging Face models', { error: error.message, query: trimmedQuery });
          setError(error.message);
        } else {
          logger.error('Unknown error searching Hugging Face models', { error, query: trimmedQuery });
          setError('Search failed.');
        }
        setResults([]);
      } finally {
        if (isActive) {
          setIsLoading(false);
        }
      }
    }, debounceMs);

    return () => {
      isActive = false;
      clearTimeout(handle);
    };
  }, [enabled, searchQuery, debounceMs, selectedSource, requestContext]);

  const hydrateModelDetails = useCallback(async (model: RemoteModelInfo): Promise<void> => {
    if (selectedSource === 'cached') return;
    if (!isAPIAvailable()) {
      return;
    }
    if (hasExactDownloadDetails(model)) {
      return;
    }

    const repoId = model.repoId;
    const existing = inFlightHydrationsRef.current.get(repoId);
    if (existing) {
      return existing;
    }

    const generation = generationRef.current;
    const request = (async () => {
      setHydratingRepoIds((prev) => {
        const next = new Set(prev);
        next.add(repoId);
        return next;
      });

      try {
        const response = await api.get_hf_download_details(repoId, model.quants);
        if (!response.success) {
          throw new APIError(response.error, 'get_hf_download_details');
        }
        if (generation !== generationRef.current) {
          return;
        }
        const details = response.details;
        if (details.repoId !== repoId) {
          throw new APIError('Download details did not match the requested repository.', 'get_hf_download_details');
        }
        const downloadOptions = details.downloadOptions.map((option) => ({
          ...option,
          fileGroup: option.fileGroup
            ? { ...option.fileGroup, filenames: [...option.fileGroup.filenames] }
            : option.fileGroup,
        }));

        setResults((prev) =>
          prev.map((entry) =>
            entry.repoId === repoId
              ? {
                  ...entry,
                  downloadOptions,
                  totalSizeBytes: details.totalSizeBytes,
                }
              : entry
          )
        );
      } catch (hydrateError) {
        const latest = resultsRef.current.find((entry) => entry.repoId === repoId);
        if (generation !== generationRef.current || !latest) {
          return;
        }

        logger.warn('Failed to hydrate Hugging Face download details', {
          repoId,
          error: hydrateError instanceof Error ? hydrateError.message : hydrateError,
        });
      } finally {
        if (generation === generationRef.current) {
          inFlightHydrationsRef.current.delete(repoId);
          setHydratingRepoIds((prev) => {
            const next = new Set(prev);
            next.delete(repoId);
            return next;
          });
        }
      }
    })();

    inFlightHydrationsRef.current.set(repoId, request);
    return request;
  }, [selectedSource]);

  return {
    results: visibleResults,
    isCachedSearch: selectedSource === 'cached',
    kinds,
    error,
    isLoading,
    hydratingRepoIds,
    hydrateModelDetails,
  };
}
