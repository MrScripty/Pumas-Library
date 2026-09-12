import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../../api/adapter';
import type { DesktopBridgeRuntimeAPI } from '../../types/api-bridge-runtime';
import type { ModelServeError, ModelServingConfig, ServedModelStatus } from '../../types/api-serving';
import type { ServingControlObservation, ServingControlStatus } from '../../hooks/useServingStatus';

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
const EMPTY_SERVED_MODELS: ServedModelStatus[] = [];
type ServingMessage = { source: 'action' | 'loaded'; text: string };

function actionMessage(text: string): ServingMessage {
  return { source: 'action', text };
}

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
  provider?: string | null;
  providerMode?: string | null;
}

export type ModelServingActionPhase = 'idle' | 'starting' | 'stopping' | 'uncertain';

export type ModelServeWithValidationResult =
  | { kind: 'missing_config'; message: string }
  | { kind: 'validation_request_failed'; message: string }
  | { kind: 'validation_failed'; error: ModelServeError }
  | { kind: 'serve_request_failed'; message: string }
  | { kind: 'load_failed'; error: ModelServeError }
  | { kind: 'loaded'; status: ServedModelStatus | null };

export async function serveModelWithValidation({
  api,
  config,
  modelId,
}: {
  api: Pick<DesktopBridgeRuntimeAPI, 'validate_model_serving_config' | 'serve_model'>;
  config: ModelServingConfig | null;
  modelId: string;
}): Promise<ModelServeWithValidationResult> {
  if (!config) {
    return {
      kind: 'missing_config',
      message: 'Select a runtime target before serving.',
    };
  }

  const request = { model_id: modelId, config };
  const validation = await api.validate_model_serving_config(request);
  const validationWire = validation as unknown as Record<string, unknown>;
  if (validationWire['success'] !== true) {
    return {
      kind: 'validation_request_failed',
      message: validation.error ?? 'Serving validation failed.',
    };
  }
  if (validationWire['valid'] !== true) {
    return {
      kind: 'validation_failed',
      error: validation.errors[0] ?? getValidationErrorFallback(modelId, config.profile_id),
    };
  }

  const response = await api.serve_model(request);
  const responseWire = response as unknown as Record<string, unknown>;
  if (responseWire['success'] !== true) {
    return {
      kind: 'serve_request_failed',
      message: response.error ?? 'Serving request failed.',
    };
  }
  if (responseWire['loaded'] === true) {
    return {
      kind: 'loaded',
      status: response.status ?? null,
    };
  }
  if (responseWire['loaded'] !== false) {
    return {
      kind: 'serve_request_failed',
      message: 'Serving response was malformed.',
    };
  }
  return {
    kind: 'load_failed',
    error: response.load_error ?? getProviderLoadFailedError(modelId, config.profile_id),
  };
}

function matchesServingTarget(
  servedModel: ServingControlStatus,
  modelId: string,
  target: ModelServingActionTarget
): boolean {
  if (servedModel.model_id !== modelId) {
    return false;
  }
  if (target.profileId && servedModel.profile_id !== target.profileId) {
    return false;
  }
  if (target.provider && servedModel.provider !== target.provider) {
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
  target: ModelServingActionTarget = {},
  servedModels: ServedModelStatus[] = EMPTY_SERVED_MODELS,
  controlObservation: ServingControlObservation = { kind: 'known', rows: servedModels }
) {
  const [actionPhase, setActionPhase] = useState<ModelServingActionPhase>('idle');
  const [message, setMessage] = useState<ServingMessage | null>(null);
  const [serveError, setServeError] = useState<ModelServeError | null>(null);
  const [servedStatus, setServedStatus] = useState<ServingControlStatus | null>(null);
  const invocationRef = useRef(0);
  const targetKey = [modelId, target.profileId, target.provider, target.providerMode, target.modelAlias]
    .map((part) => part ?? '')
    .join('\u0000');
  const targetKeyRef = useRef(targetKey);
  const loadedRef = useRef(false);
  const actionPhaseRef = useRef<ModelServingActionPhase>('idle');
  const actionDirectionRef = useRef<'start' | 'stop' | null>(null);
  const controlRows = controlObservation.kind === 'known' ? controlObservation.rows : null;
  actionPhaseRef.current = actionPhase;

  useEffect(() => {
    if (targetKeyRef.current === targetKey) return;
    targetKeyRef.current = targetKey;
    invocationRef.current += 1;
    loadedRef.current = false;
    actionDirectionRef.current = null;
    setActionPhase('idle');
    setMessage(null);
    setServeError(null);
    setServedStatus(null);
  }, [targetKey]);

  useEffect(() => () => {
    invocationRef.current += 1;
  }, []);

  useEffect(() => {
    if (!controlRows) {
      loadedRef.current = false;
      setServedStatus(null);
      return;
    }
    const matchingRows = controlRows.filter((servedModel) =>
      matchesServingTarget(servedModel, modelId, target)
    );
    const status = matchingRows.find(
      (servedModel) =>
        servedModel.load_state === 'loaded'
    );
    const interruptedStatus = matchingRows.find(
      (servedModel) =>
        servedModel.load_state === 'failed' && servedModel.last_error?.code === 'unknown'
    );
    const terminalStatus = matchingRows.find(
      (servedModel) =>
        (servedModel.load_state === 'failed' && servedModel.last_error?.code !== 'unknown') ||
        servedModel.load_state === 'unloaded'
    );
    loadedRef.current = Boolean(status);
    setServedStatus(status ?? null);
    if (status) {
      if (actionPhaseRef.current === 'starting' || actionPhaseRef.current === 'uncertain') {
        invocationRef.current += 1;
        setActionPhase('idle');
      }
      setMessage({ source: 'loaded', text: `Loaded on ${status.profile_id}` });
    } else {
      setMessage((current) => (current?.source === 'loaded' ? null : current));
      setActionPhase((current) => {
        if (interruptedStatus) {
          invocationRef.current += 1;
          return 'uncertain';
        }
        if (terminalStatus) {
          invocationRef.current += 1;
          return 'idle';
        }
        if (
          (current === 'stopping' || (current === 'uncertain' && actionDirectionRef.current === 'stop')) &&
          matchingRows.length === 0
        ) {
          invocationRef.current += 1;
          return 'idle';
        }
        if (current === 'idle' && matchingRows.some((row) =>
          (row.load_state === 'requested' || row.load_state === 'loading' || row.load_state === 'unloading')
        )) return 'uncertain';
        return current;
      });
    }
  }, [controlRows, modelId, target.modelAlias, target.profileId, target.provider, targetKey]);

  const serveModel = useCallback(
    async (config: ModelServingConfig | null) => {
      if (loadedRef.current) {
        return;
      }
      if (!config) {
        setMessage(actionMessage('Select a runtime target before serving.'));
        return;
      }

      const api = getElectronAPI();
      if (!api) {
        setMessage(actionMessage('Serving API is not available in this app session.'));
        return;
      }

      const invocation = ++invocationRef.current;
      const invocationTarget = targetKeyRef.current;
      actionDirectionRef.current = 'start';
      setActionPhase('starting');
      setMessage(actionMessage('Starting serving...'));
      setServeError(null);

      try {
        const result = await serveModelWithValidation({ api, config, modelId });
        if (invocation !== invocationRef.current || invocationTarget !== targetKeyRef.current) return;
        if (result.kind === 'missing_config') {
          setMessage(actionMessage(result.message));
          setActionPhase('idle');
          return;
        }
        if (result.kind === 'validation_request_failed') {
          setMessage(actionMessage(result.message));
          setActionPhase('idle');
          return;
        }
        if (result.kind === 'serve_request_failed') {
          setMessage(actionMessage(result.message));
          setActionPhase('uncertain');
          return;
        }
        if (result.kind === 'validation_failed' || result.kind === 'load_failed') {
          setServeError(result.error);
          setMessage(null);
          setActionPhase('idle');
          return;
        }
        setActionPhase('starting');
      } catch (caught) {
        if (invocation !== invocationRef.current || invocationTarget !== targetKeyRef.current) return;
        setMessage(
          actionMessage(caught instanceof Error ? caught.message : 'Serving request failed')
        );
        setActionPhase('uncertain');
      }
    },
    [modelId, targetKey]
  );

  const unloadModel = useCallback(async () => {
    const api = getElectronAPI();
    if (!api?.unserve_model || !servedStatus) {
      return;
    }

    const invocation = ++invocationRef.current;
    const invocationTarget = targetKeyRef.current;
    actionDirectionRef.current = 'stop';
    setActionPhase('stopping');
    setMessage(null);
    setServeError(null);

    try {
      const response = await api.unserve_model({
        model_id: servedStatus.model_id,
        provider: servedStatus.provider,
        profile_id: servedStatus.profile_id,
        model_alias: servedStatus.model_alias ?? null,
      });
      if (invocation !== invocationRef.current || invocationTarget !== targetKeyRef.current) return;
      if (!response.unloaded) {
        setMessage(actionMessage(response.error ?? 'Model was not loaded'));
        setActionPhase('uncertain');
      }
    } catch (caught) {
      if (invocation !== invocationRef.current || invocationTarget !== targetKeyRef.current) return;
      setMessage(actionMessage(caught instanceof Error ? caught.message : 'Unload request failed'));
      setActionPhase('uncertain');
    }
  }, [servedStatus, targetKey]);

  return {
    actionPhase,
    isLoading: Boolean(controlRows?.some(
      (servedModel) =>
        matchesServingTarget(servedModel, modelId, target) &&
        (servedModel.load_state === 'requested' || servedModel.load_state === 'loading')
    )),
    isUnavailable: Boolean(controlRows?.some(
      (servedModel) =>
        matchesServingTarget(servedModel, modelId, target) &&
        servedModel.load_state === 'failed' &&
        servedModel.last_error?.code === 'unknown'
    )),
    isSubmitting: actionPhase === 'starting' || actionPhase === 'stopping',
    message: message?.text ?? null,
    serveError,
    servedStatus,
    serveModel,
    unloadModel,
  };
}
