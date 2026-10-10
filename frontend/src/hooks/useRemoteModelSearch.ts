/**
 * Remote Model Search Hook
 *
 * Handles searching Hugging Face for models with debouncing.
 * Extracted from ModelManager.tsx
 */

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { RemoteModelInfo, RemoteSearchSource } from '../types/apps';
import { effectiveSearchSource, presentSearchResults } from '../utils/hfCachedDiscovery';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';
import { hasExactDownloadDetails } from '../utils/hfDownloadDetails';

const logger = getLogger('useRemoteModelSearch');
const EMPTY_RESULTS: RemoteModelInfo[] = [];

interface UseRemoteModelSearchOptions {
  enabled: boolean;
  searchQuery: string;
  debounceMs?: number;
  source?: RemoteSearchSource;
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
  const [hydratedRepoIds, setHydratedRepoIds] = useState<Set<string>>(new Set());
  const [hydrationErrors, setHydrationErrors] = useState<Record<string, string>>({});
  const generationRef = useRef(0);
  const resultsRef = useRef<RemoteModelInfo[]>([]);
  const hydratedRepoIdsRef = useRef<Set<string>>(new Set());
  const hydrationContextRef = useRef<string | null>(null);
  const inFlightHydrationsRef = useRef<Map<string, Promise<void>>>(new Map());
  const selectedSource = effectiveSearchSource(searchQuery, source);
  const requestContext = JSON.stringify([enabled, selectedSource, searchQuery.trim()]);
  const visibleResults = resultContext === requestContext ? results : EMPTY_RESULTS;

  useEffect(() => () => {
    generationRef.current += 1;
    inFlightHydrationsRef.current.clear();
  }, []);

  useLayoutEffect(() => {
    resultsRef.current = visibleResults;
  }, [visibleResults]);

  useLayoutEffect(() => {
    hydrationContextRef.current = requestContext;
    return () => { hydrationContextRef.current = null; };
  }, [requestContext]);

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
    setHydratedRepoIds(new Set());
    hydratedRepoIdsRef.current.clear();
    setHydrationErrors({});
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
        // Discovery may reuse local details, but must not fetch a tree/config
        // for each result. A selected menu requests its details separately.
        const result = await api.search_hf_models(query, null, 25, 0);
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
    if (!enabled || selectedSource === 'cached' || hydrationContextRef.current !== requestContext) return;
    const currentModel = resultsRef.current.find(entry => entry.repoId === model.repoId);
    if (!currentModel || hasExactDownloadDetails(currentModel) || hydratedRepoIdsRef.current.has(model.repoId)) {
      return;
    }
    if (!isAPIAvailable()) {
      setHydrationErrors(prev => ({ ...prev, [model.repoId]: 'Hugging Face download details are unavailable.' }));
      return;
    }

    const repoId = model.repoId;
    const existing = inFlightHydrationsRef.current.get(repoId);
    if (existing) {
      return existing;
    }

    const generation = generationRef.current;
    const isCurrent = () => generation === generationRef.current && hydrationContextRef.current === requestContext;
    // Register before invocation: even a synchronous transport throw must
    // settle the registered request, so a later explicit retry remains usable.
    const request = Promise.resolve().then(async () => {
      if (!isCurrent()) return;
      setHydrationErrors(prev => {
        const next = { ...prev };
        delete next[repoId];
        return next;
      });
      setHydratingRepoIds((prev) => {
        const next = new Set(prev);
        next.add(repoId);
        return next;
      });

      try {
        const response = await api.get_hf_download_details(repoId, currentModel.quants);
        if (!response.success) {
          throw new APIError(response.error, 'get_hf_download_details');
        }
        if (!isCurrent()) {
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
        hydratedRepoIdsRef.current.add(repoId);
        setHydratedRepoIds(new Set(hydratedRepoIdsRef.current));

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
        if (!isCurrent() || !latest) {
          return;
        }

        logger.warn('Failed to hydrate Hugging Face download details', {
          repoId,
          error: hydrateError instanceof Error ? hydrateError.message : hydrateError,
        });
        setHydrationErrors(prev => ({ ...prev, [repoId]: hydrateError instanceof Error
          ? hydrateError.message : 'Download details are unavailable.' }));
      } finally {
        if (isCurrent()) {
          inFlightHydrationsRef.current.delete(repoId);
          setHydratingRepoIds((prev) => {
            const next = new Set(prev);
            next.delete(repoId);
            return next;
          });
        }
      }
    });

    inFlightHydrationsRef.current.set(repoId, request);
    return request;
  }, [enabled, selectedSource, requestContext]);

  return {
    results: visibleResults,
    isCachedSearch: selectedSource === 'cached',
    kinds,
    error,
    isLoading,
    hydratingRepoIds,
    hydratedRepoIds,
    hydrationErrors,
    hydrateModelDetails,
  };
}
