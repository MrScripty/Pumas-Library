/**
 * Remote Model Search Hook
 *
 * Handles searching Hugging Face for models with debouncing.
 * Extracted from ModelManager.tsx
 */

import { useState, useEffect, useMemo } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { RemoteModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useRemoteModelSearch');

interface UseRemoteModelSearchOptions {
  enabled: boolean;
  searchQuery: string;
  debounceMs?: number;
}

export function useRemoteModelSearch({
  enabled,
  searchQuery,
  debounceMs = 300,
}: UseRemoteModelSearchOptions) {
  const [results, setResults] = useState<RemoteModelInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

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
        const result = await api.search_hf_models(trimmedQuery, null, 25);
        if (!isActive) {
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

  return {
    results,
    kinds,
    error,
    isLoading,
  };
}
