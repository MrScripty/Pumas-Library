import { useCallback, useEffect, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { BaseResponse, LaunchResponse } from '../types/api';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

type ProcessTransition = 'starting' | 'stopping' | null;

interface UseManagedProcessOptions<TLaunchResponse extends LaunchResponse, TStopResponse extends BaseResponse> {
  appName: string;
  isRunning: boolean;
  launch: () => Promise<TLaunchResponse>;
  stop: () => Promise<TStopResponse>;
  onLaunchSuccess?: (result: TLaunchResponse) => Promise<void> | void;
}

interface ManagedProcessState {
  launchError: string | null;
  launchLogPath: string | null;
  isStarting: boolean;
  isStopping: boolean;
  openLogPath: (path: string | null | undefined) => Promise<void>;
}

function logProcessError(
  loggerName: string,
  action: string,
  appName: string,
  error: unknown,
  extra: Record<string, unknown> = {}
): void {
  const logger = getLogger(loggerName);
  if (error instanceof APIError) {
    logger.error(`API error ${action} ${appName}`, { error: error.message, endpoint: error.endpoint, ...extra });
  } else if (error instanceof Error) {
    logger.error(`Unexpected error ${action} ${appName}`, { error: error.message, ...extra });
  } else {
    logger.error(`Unknown error ${action} ${appName}`, { error, ...extra });
  }
}

export function useManagedProcess<TLaunchResponse extends LaunchResponse, TStopResponse extends BaseResponse>({
  appName,
  isRunning,
  launch,
  stop,
  onLaunchSuccess,
}: UseManagedProcessOptions<TLaunchResponse, TStopResponse>) {
  const loggerName = `use${appName}Process`;
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchLogPath, setLaunchLogPath] = useState<string | null>(null);
  const [transition, setTransition] = useState<ProcessTransition>(null);

  useEffect(() => {
    if (transition === 'starting' && isRunning) {
      setTransition(null);
      return;
    }

    if (transition === 'stopping' && !isRunning) {
      setTransition(null);
    }
  }, [isRunning, transition]);

  const startProcess = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setTransition('starting');
    try {
      const result = await launch();

      if (result.success) {
        setLaunchError(null);
        setLaunchLogPath(result.log_path || null);
        if (onLaunchSuccess) {
          await onLaunchSuccess(result);
        }
      } else {
        setLaunchError(result.error || `Failed to launch ${appName}`);
        setLaunchLogPath(result.log_path || null);
        setTransition(null);
      }
    } catch (error) {
      setLaunchError(`Error trying to launch ${appName}`);
      setTransition(null);
      logProcessError(loggerName, 'launching', appName, error);
    }
  }, [appName, launch, loggerName, onLaunchSuccess]);

  const stopProcess = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setTransition('stopping');
    try {
      const result = await stop();

      if (result.success) {
        setLaunchError(null);
      } else {
        setLaunchError(`Failed to stop ${appName}`);
        setTransition(null);
      }
    } catch (error) {
      setLaunchError(`Error trying to stop ${appName}`);
      setTransition(null);
      logProcessError(loggerName, 'stopping', appName, error);
    }
  }, [appName, loggerName, stop]);

  const openLogPath = useCallback(async (path: string | null | undefined) => {
    if (!path || !isAPIAvailable()) {
      return;
    }

    try {
      await api.open_path(path);
    } catch (error) {
      logProcessError(loggerName, 'opening log path for', appName, error, { path });
    }
  }, [appName, loggerName]);

  const state: ManagedProcessState = {
    launchError,
    launchLogPath,
    isStarting: transition === 'starting',
    isStopping: transition === 'stopping',
    openLogPath,
  };

  return {
    ...state,
    startProcess,
    stopProcess,
  };
}
