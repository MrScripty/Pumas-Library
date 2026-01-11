/**
 * Models management hook
 *
 * Handles model fetching, scanning, organization, and FTS search.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { modelsAPI } from '../api/models';
import { importAPI } from '../api/import';
import type { ModelCategory, ModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useModels');

/** Debounce delay for search queries (ms) */
const SEARCH_DEBOUNCE_MS = 300;

export function useModels() {
  const [modelGroups, setModelGroups] = useState<ModelCategory[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchQueryTime, setSearchQueryTime] = useState<number | null>(null);
  const modelCountRef = useRef<number | null>(null);
  const isModelCountPolling = useRef(false);
  const searchSequenceRef = useRef(0);
  const lastRenderedSequenceRef = useRef(0);
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const fetchModels = useCallback(async () => {
    try {
      const result = await modelsAPI.getModels();
      if (result.success && result.models) {
        // Transform backend models to frontend ModelCategory structure
        const categorizedModels: ModelCategory[] = [];
        const categoryMap = new Map<string, ModelInfo[]>();

        // Group models by category
        const modelEntries = Object.entries(result.models);
        modelEntries.forEach(([path, modelData]: [string, any]) => {
          const category = modelData.modelType || 'uncategorized';
          const fileName = path.split('/').pop() || path;
          const displayName = modelData.officialName || modelData.cleanedName || fileName;

          const modelInfo: ModelInfo = {
            id: path,
            name: displayName,
            category: category,
            path: path,
            size: modelData.size,
            date: modelData.addedDate,
          };

          if (!categoryMap.has(category)) {
            categoryMap.set(category, []);
          }
          categoryMap.get(category)!.push(modelInfo);
        });

        // Convert map to array format
        categoryMap.forEach((models, category) => {
          categorizedModels.push({ category, models });
        });

        setModelGroups(categorizedModels);
        modelCountRef.current = modelEntries.length;
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching models', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching models', { error: error.message });
      } else {
        logger.error('Unknown error fetching models', { error });
      }
    }
  }, []);

  const scanModels = useCallback(async () => {
    try {
      const result = await modelsAPI.scanSharedStorage();
      if (result.success) {
        await fetchModels();
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error scanning models', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error scanning models', { error: error.message });
      } else {
        logger.error('Unknown error scanning models', { error });
      }
    }
  }, [fetchModels]);

  // Poll for model count changes
  useEffect(() => {
    const pollModelLibrary = async () => {
      if (isModelCountPolling.current) {
        return;
      }

      isModelCountPolling.current = true;
      try {
        const result = await modelsAPI.scanSharedStorage();
        if (result.success) {
          const modelsFound = result.result?.modelsFound;
          if (typeof modelsFound === 'number') {
            if (modelCountRef.current === null) {
              modelCountRef.current = modelsFound;
            } else if (modelsFound !== modelCountRef.current) {
              modelCountRef.current = modelsFound;
              await fetchModels();
            }
          }
        }
      } catch (error) {
        if (error instanceof APIError) {
          logger.error('API error polling model library count', { error: error.message, endpoint: error.endpoint });
        } else if (error instanceof Error) {
          logger.error('Unexpected error polling model library count', { error: error.message });
        } else {
          logger.error('Unknown error polling model library count', { error });
        }
      } finally {
        isModelCountPolling.current = false;
      }
    };

    const interval = setInterval(() => {
      void pollModelLibrary();
    }, 10000);

    return () => clearInterval(interval);
  }, [fetchModels]);

  // Initial fetch
  useEffect(() => {
    void fetchModels();
  }, [fetchModels]);

  /**
   * Debounced FTS search for models.
   * Uses sequence guards to discard stale responses.
   */
  const searchModelsFTS = useCallback(
    (query: string, modelType?: string | null, tags?: string[] | null) => {
      // Clear any pending search
      if (searchTimeoutRef.current) {
        clearTimeout(searchTimeoutRef.current);
      }

      // Empty query - reset to full list
      if (!query.trim()) {
        setIsSearching(false);
        setSearchQueryTime(null);
        void fetchModels();
        return;
      }

      // Increment sequence for this search
      const currentSequence = ++searchSequenceRef.current;

      // Debounce the search
      searchTimeoutRef.current = setTimeout(async () => {
        setIsSearching(true);

        try {
          const result = await importAPI.searchModelsFTS(query, 100, 0, modelType, tags);

          // Sequence guard: discard stale responses
          if (currentSequence < lastRenderedSequenceRef.current) {
            logger.debug('Discarding stale search response', {
              currentSequence,
              lastRendered: lastRenderedSequenceRef.current,
            });
            return;
          }

          lastRenderedSequenceRef.current = currentSequence;

          if (result.success && result.models) {
            // Transform FTS results to ModelCategory format
            const categoryMap = new Map<string, ModelInfo[]>();

            for (const model of result.models) {
              const category = model.model_type || 'uncategorized';
              const modelInfo: ModelInfo = {
                id: model.model_id,
                name: model.official_name,
                category: category,
                path: model.file_path,
                size: model.size_bytes,
                date: model.added_date,
              };

              if (!categoryMap.has(category)) {
                categoryMap.set(category, []);
              }
              categoryMap.get(category)!.push(modelInfo);
            }

            const categorizedModels: ModelCategory[] = [];
            categoryMap.forEach((models, category) => {
              categorizedModels.push({ category, models });
            });

            setModelGroups(categorizedModels);
            setSearchQueryTime(result.query_time_ms);
          }
        } catch (error) {
          if (error instanceof APIError) {
            logger.error('API error in FTS search', {
              error: error.message,
              endpoint: error.endpoint,
            });
          } else if (error instanceof Error) {
            logger.error('Unexpected error in FTS search', { error: error.message });
          } else {
            logger.error('Unknown error in FTS search', { error });
          }
        } finally {
          setIsSearching(false);
        }
      }, SEARCH_DEBOUNCE_MS);
    },
    [fetchModels]
  );

  // Cleanup search timeout on unmount
  useEffect(() => {
    return () => {
      if (searchTimeoutRef.current) {
        clearTimeout(searchTimeoutRef.current);
      }
    };
  }, []);

  return {
    modelGroups,
    fetchModels,
    scanModels,
    searchModelsFTS,
    isSearching,
    searchQueryTime,
  };
}
