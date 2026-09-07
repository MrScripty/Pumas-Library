import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api/adapter';
import type { ConversionDirection, ConversionProgress, ConversionSetupStatusResponse, ConversionStatus } from '../types/api-conversion';

export function isConversionTerminal(status: ConversionStatus): boolean {
  return status === 'completed' || status === 'cancelled' || status === 'error';
}

interface Options {
  modelId: string;
  direction: ConversionDirection;
  onCompleted?: () => void;
}

interface State {
  ready: boolean | null;
  conversions: readonly ConversionProgress[];
  loading: boolean;
  busy: boolean;
  error: string | null;
  startUncertain: boolean;
  setupUncertain: boolean;
  setupOperation: ConversionSetupStatusResponse['setup'];
  awaitingProgress: boolean;
}

interface Controller {
  refresh: () => void;
  setup: () => Promise<void>;
  start: () => Promise<void>;
  cancel: (id: string) => Promise<void>;
}

const initialState: State = { ready: null, conversions: [], loading: true, busy: false, error: null, startUncertain: false, setupUncertain: false, setupOperation: null, awaitingProgress: false };
const uncertainMessage = 'The conversion start outcome is unknown. Inspect conversion status, then close and reopen this dialog before retrying.';
const readErrorMessage = 'Could not read conversion status. Refresh status to retry.';
const setupUncertainMessage = 'Tool setup could not be confirmed and may still be running. Refresh status to check backend setup status.';

export function useModelConversionWorkflow({ modelId, direction, onCompleted }: Options) {
  const [state, setState] = useState<State>(initialState);
  const controller = useRef<Controller | null>(null);
  // One queue also spans dependency changes: superseded calls are observed
  // before the next scope starts reads; the bridge has no cancellation API.
  const tail = useRef<Promise<void>>(Promise.resolve());
  const uncertainStart = useRef(false);
  const uncertainSetup = useRef(false);
  const uncertainSetupPrevious = useRef<string | null>(null);
  const completedCallback = useRef(onCompleted);
  completedCallback.current = onCompleted;

  useEffect(() => {
    let disposed = false;
    const isCurrent = () => !disposed;
    let pending = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let current = { ...initialState, startUncertain: uncertainStart.current, setupUncertain: uncertainSetup.current,
      error: uncertainStart.current ? uncertainMessage : uncertainSetup.current ? setupUncertainMessage : null };
    let baseline = false;
    let acceptedStartId: string | null = null;
    const completed = new Set<string>();
    const publish = (changes: Partial<State>) => {
      if (disposed) return;
      current = { ...current, ...changes };
      setState(current);
    };
    const stopTimer = () => { clearTimeout(timer); timer = undefined; };
    const acceptSetup = (setupOperation: State['setupOperation']) => {
      if (disposed) return;
      if (uncertainSetup.current && setupOperation !== null &&
        (setupOperation.status === 'in_progress' || setupOperation.operationId !== uncertainSetupPrevious.current)) {
        uncertainSetup.current = false;
        publish({ setupUncertain: false, error: current.startUncertain ? uncertainMessage : null });
      }
      publish({ setupOperation, setupUncertain: uncertainSetup.current,
        ...(uncertainSetup.current ? { error: current.startUncertain ? uncertainMessage : setupUncertainMessage } : {}) });
    };
    const read = async (checkReadiness = true) => {
      publish({ loading: true });
      const status = await api.get_conversion_setup();
      if (!isCurrent()) return;
      const previousSetup = current.setupOperation;
      const terminalChanged = status.setup !== null && status.setup.status !== 'in_progress' &&
        (previousSetup === null || previousSetup.operationId !== status.setup.operationId || previousSetup.status === 'in_progress');
      acceptSetup(status.setup);
      if (checkReadiness || terminalChanged) {
        const readiness = await api.check_conversion_environment();
        if (!isCurrent()) return;
        publish({ ready: readiness.ready });
      }
      const listed = await api.list_model_conversions();
      if (!isCurrent()) return;
      const conversions = listed.conversions.filter((entry) => entry.sourceModelId === modelId);
      if (conversions.some((entry) => entry.conversionId === acceptedStartId && isConversionTerminal(entry.status))) acceptedStartId = null;
      publish({ conversions, awaitingProgress: acceptedStartId !== null && !conversions.some((entry) => entry.conversionId === acceptedStartId) });
      for (const entry of conversions) {
        if (entry.status !== 'completed' || completed.has(entry.conversionId)) continue;
        completed.add(entry.conversionId);
        if (baseline) completedCallback.current?.();
      }
      baseline = true;
    };
    const run = (operation: () => Promise<void>, mutating = false, errorMessage = readErrorMessage): Promise<void> => {
      if (disposed || pending) return Promise.resolve();
      pending = true;
      stopTimer();
      publish({ busy: mutating, loading: !mutating, error: current.startUncertain ? uncertainMessage : current.setupUncertain ? setupUncertainMessage : null });
      const result = tail.current.then(async () => {
        if (disposed) return;
        let succeeded = false;
        try {
          await operation();
          succeeded = true;
        } catch {
          // Rejection is observed even for a superseded scope, but cannot
          // update that scope's replacement or trigger another operation.
          if (isCurrent()) publish({
            error: current.startUncertain ? uncertainMessage : current.setupUncertain ? setupUncertainMessage : errorMessage,
          });
        } finally {
          pending = false;
          if (isCurrent()) {
            publish({ busy: false, loading: false });
            if (succeeded && (current.setupOperation?.status === 'in_progress' || acceptedStartId !== null || current.conversions.some((entry) => !isConversionTerminal(entry.status)))) {
              timer = setTimeout(() => { void run(() => read(false)); }, 1000);
            }
          }
        }
      });
      tail.current = result;
      return result;
    };
    const scope: Controller = {
      refresh: () => { void run(read); },
      setup: () => {
        if (current.ready !== false || current.error !== null || current.setupUncertain || current.setupOperation?.status === 'in_progress' || acceptedStartId !== null || current.conversions.some((entry) => !isConversionTerminal(entry.status))) return Promise.resolve();
        return run(async () => {
          const previous = current.setupOperation?.operationId ?? null;
          try {
            const admitted = await api.start_conversion_setup(previous);
            acceptSetup(admitted.setup);
            publish({ busy: false });
          } catch (error) {
            uncertainSetup.current = true;
            uncertainSetupPrevious.current = previous;
            publish({ setupUncertain: true });
            throw error;
          }
          if (!disposed) await read();
        }, true);
      },
      start: () => {
        if (current.ready !== true || current.error !== null || current.startUncertain || current.setupUncertain || current.setupOperation?.status === 'in_progress' || acceptedStartId !== null || current.conversions.some((entry) => !isConversionTerminal(entry.status))) return Promise.resolve();
        if (direction !== 'gguf_to_safetensors' && direction !== 'safetensors_to_gguf') {
          publish({ error: 'This dialog supports only GGUF and safetensors format conversion.' });
          return Promise.resolve();
        }
        return run(async () => {
          try {
            const result = await api.start_model_conversion(modelId, direction, 'F16');
            acceptedStartId = result.conversion_id;
            publish({ awaitingProgress: true });
          } catch (error) {
            uncertainStart.current = true;
            publish({ startUncertain: true });
            throw error;
          }
          if (!disposed) await read(false);
        }, true);
      },
      cancel: (id) => {
        if (!current.conversions.some((entry) => entry.conversionId === id && !isConversionTerminal(entry.status))) return Promise.resolve();
        return run(async () => {
          const result = await api.cancel_model_conversion(id);
          if (disposed) return;
          await read(false);
          if (!result.cancelled) publish({ error: 'This conversion is not active; cancellation was not requested.' });
        }, true, 'Could not request cancellation. Refresh status to check the backend.');
      },
    };
    controller.current = scope;
    setState(current);
    scope.refresh();
    return () => {
      disposed = true;
      stopTimer();
      if (controller.current === scope) controller.current = null;
    };
  }, [modelId, direction]);

  return {
    ...state,
    refresh: useCallback(() => { controller.current?.refresh(); }, []),
    setup: useCallback(() => controller.current?.setup() ?? Promise.resolve(), []),
    start: useCallback(() => controller.current?.start() ?? Promise.resolve(), []),
    cancel: useCallback((id: string) => controller.current?.cancel(id) ?? Promise.resolve(), []),
  };
}
