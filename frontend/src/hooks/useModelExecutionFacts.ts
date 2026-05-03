import { useEffect, useState } from 'react';
import { modelsAPI } from '../api/models';
import type { MetadataSource } from '../components/ModelMetadataFieldConfig';
import type { ResolvedModelPackageFacts } from '../types/api';

export function useModelExecutionFacts(modelId: string, activeSource: MetadataSource) {
  const [executionFacts, setExecutionFacts] = useState<ResolvedModelPackageFacts | null>(null);
  const [executionFactsLoading, setExecutionFactsLoading] = useState(false);
  const [executionFactsError, setExecutionFactsError] = useState<string | null>(null);

  useEffect(() => {
    setExecutionFacts(null);
    setExecutionFactsLoading(false);
    setExecutionFactsError(null);
  }, [modelId]);

  useEffect(() => {
    if (
      activeSource !== 'execution' ||
      executionFacts ||
      executionFactsLoading ||
      executionFactsError
    ) {
      return;
    }

    let cancelled = false;
    setExecutionFactsLoading(true);
    setExecutionFactsError(null);
    modelsAPI
      .resolveModelPackageFacts(modelId)
      .then((facts) => {
        if (!cancelled) {
          setExecutionFacts(facts);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setExecutionFactsError(e instanceof Error ? e.message : 'Failed to load execution facts');
        }
      })
      .finally(() => {
        if (!cancelled) {
          setExecutionFactsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeSource, executionFacts, executionFactsError, modelId]);

  return { executionFacts, executionFactsError, executionFactsLoading };
}
