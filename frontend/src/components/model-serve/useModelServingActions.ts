import { useCallback, useEffect, useState } from 'react';
import { getElectronAPI } from '../../api/adapter';
import type { ModelServeError, ModelServingConfig, ServedModelStatus } from '../../types/api-serving';

const INVALID_CONFIGURATION_ERROR = {
  code: 'invalid_request',
  severity: 'non_critical',
  message: 'The selected runtime target cannot serve this model configuration.',
} as const;

const PROVIDER_LOAD_FAILED_ERROR = {
  code: 'provider_load_failed',
  severity: 'non_critical',
  message: 'The runtime did not report the model as loaded.',
} as const;

function getValidationErrorFallback(modelId: string, profileId: string): ModelServeError {
  return {
    ...INVALID_CONFIGURATION_ERROR,
    model_id: modelId,
    profile_id: profileId,
  };
}

function getProviderLoadFailedError(modelId: string, profileId: string): ModelServeError {
  return {
    ...PROVIDER_LOAD_FAILED_ERROR,
    model_id: modelId,
    profile_id: profileId,
  };
}

export interface ModelServingActionTarget {
  modelAlias?: string | null;
  profileId?: string | null;
}

function matchesServingTarget(
  servedModel: ServedModelStatus,
  modelId: string,
  target: ModelServingActionTarget
): boolean {
  if (servedModel.model_id !== modelId) {
    return false;
  }
  if (target.profileId && servedModel.profile_id !== target.profileId) {
    return false;
  }
  if (
    target.modelAlias !== undefined &&
    (servedModel.model_alias ?? null) !== target.modelAlias
  ) {
    return false;
  }
  return true;
}

export function useModelServingActions(
  modelId: string,
  target: ModelServingActionTarget = {}
) {
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [serveError, setServeError] = useState<ModelServeError | null>(null);
  const [servedStatus, setServedStatus] = useState<ServedModelStatus | null>(null);

  useEffect(() => {
    const api = getElectronAPI();
    if (!api?.get_serving_status) {
      return;
    }

    let isActive = true;
    void api.get_serving_status().then((response) => {
      if (!isActive || !response.success) {
        return;
      }
      const status = response.snapshot.served_models.find((servedModel) =>
        matchesServingTarget(servedModel, modelId, target)
      );
      setServedStatus(status ?? null);
      if (status) {
        setMessage(`Loaded on ${status.profile_id}`);
      }
    });

    return () => {
      isActive = false;
    };
  }, [modelId, target.modelAlias, target.profileId]);

  const serveModel = useCallback(
    async (config: ModelServingConfig | null) => {
      if (!config) {
        setMessage('Select a runtime target before serving.');
        return;
      }

      const api = getElectronAPI();
      if (!api) {
        setMessage('Serving API is not available in this app session.');
        return;
      }

      setIsSubmitting(true);
      setMessage('Starting serving...');
      setServeError(null);

      try {
        const request = { model_id: modelId, config };
        const validation = await api.validate_model_serving_config(request);
        if (!validation.success) {
          setMessage(validation.error ?? 'Serving validation failed.');
          return;
        }
        if (!validation.valid) {
          setServeError(
            validation.errors[0] ?? getValidationErrorFallback(modelId, config.profile_id)
          );
          setMessage(null);
          return;
        }

        const response = await api.serve_model(request);
        if (!response.success) {
          setMessage(response.error ?? 'Serving request failed.');
          return;
        }
        if (response.loaded) {
          setServedStatus(response.status ?? null);
          setMessage('Loaded');
          return;
        }
        setServeError(
          response.load_error ?? getProviderLoadFailedError(modelId, config.profile_id)
        );
        setMessage(null);
      } catch (caught) {
        setMessage(caught instanceof Error ? caught.message : 'Serving request failed');
      } finally {
        setIsSubmitting(false);
      }
    },
    [modelId]
  );

  const unloadModel = useCallback(async () => {
    const api = getElectronAPI();
    if (!api?.unserve_model || !servedStatus) {
      return;
    }

    setIsSubmitting(true);
    setMessage(null);
    setServeError(null);

    try {
      const response = await api.unserve_model({
        model_id: servedStatus.model_id,
        profile_id: servedStatus.profile_id,
        model_alias: servedStatus.model_alias ?? null,
      });
      if (response.unloaded) {
        setServedStatus(null);
        setMessage('Unloaded');
      } else {
        setMessage(response.error ?? 'Model was not loaded');
      }
    } catch (caught) {
      setMessage(caught instanceof Error ? caught.message : 'Unload request failed');
    } finally {
      setIsSubmitting(false);
    }
  }, [servedStatus]);

  return {
    isSubmitting,
    message,
    serveError,
    servedStatus,
    serveModel,
    unloadModel,
  };
}
