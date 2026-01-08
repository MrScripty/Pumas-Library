/**
 * Models management hook
 *
 * Handles model fetching, scanning, and organization.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { modelsAPI } from '../api/models';
import type { ModelCategory, ModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useModels');

export function useModels() {
  const [modelGroups, setModelGroups] = useState<ModelCategory[]>([]);
  const modelCountRef = useRef<number | null>(null);
  const isModelCountPolling = useRef(false);

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

  return {
    modelGroups,
    fetchModels,
    scanModels,
  };
}
