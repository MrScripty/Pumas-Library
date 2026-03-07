/**
 * Remote Model Search Hook
 *
 * Handles searching Hugging Face for models with debouncing.
 * Extracted from ModelManager.tsx
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { RemoteModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useRemoteModelSearch');
const DEFAULT_HYDRATE_LIMIT = 6;

interface UseRemoteModelSearchOptions {
  enabled: boolean;
  searchQuery: string;
  debounceMs?: number;
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
}: UseRemoteModelSearchOptions) {
  const [results, setResults] = useState<RemoteModelInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [hydratingRepoIds, setHydratingRepoIds] = useState<Set<string>>(new Set());
  const generationRef = useRef(0);
  const resultsRef = useRef<RemoteModelInfo[]>([]);
  const inFlightHydrationsRef = useRef<Map<string, Promise<void>>>(new Map());

  useEffect(() => {
    resultsRef.current = results;
  }, [results]);

  // Get unique kinds from results
  const kinds = useMemo(() => {
    const kindSet = new Set<string>();
    results.forEach((model) => {
      if (model.kind && model.kind !== 'unknown') {
        kindSet.add(model.kind);
      }
    });
    return ['all', ...Array.from(kindSet).sort()];
  }, [results]);

  useEffect(() => {
    generationRef.current += 1;
    inFlightHydrationsRef.current.clear();
    setHydratingRepoIds(new Set());

    if (!enabled) {
      return;
    }

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
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
        const result = await api.search_hf_models(trimmedQuery, null, 25, DEFAULT_HYDRATE_LIMIT);
        if (!isActive || generation !== generationRef.current) {
          return;
        }
        if (result.success) {
          setResults(result.models as RemoteModelInfo[]);
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
  }, [enabled, searchQuery, debounceMs]);

  const hydrateModelDetails = useCallback(async (model: RemoteModelInfo): Promise<void> => {
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
        if (!response.success || !response.details) {
          throw new APIError(response.error || 'Failed to load download details.', 'get_hf_download_details');
        }
        if (generation !== generationRef.current) {
          return;
        }

        setResults((prev) =>
          prev.map((entry) =>
            entry.repoId === repoId
              ? {
                  ...entry,
                  downloadOptions: response.details?.downloadOptions ?? entry.downloadOptions,
                  totalSizeBytes: response.details?.totalSizeBytes ?? entry.totalSizeBytes,
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
        inFlightHydrationsRef.current.delete(repoId);
        if (generation === generationRef.current) {
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
  }, []);

  return {
    results,
    kinds,
    error,
    isLoading,
    hydratingRepoIds,
    hydrateModelDetails,
  };
}
