import { useCallback, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError, ProcessError } from '../errors';
import { getLogger } from '../utils/logger';

const logger = getLogger('useDependencyInstaller');

type UseDependencyInstallerOptions = {
  refetchStatus: () => Promise<void>;
};

export function useDependencyInstaller({ refetchStatus }: UseDependencyInstallerOptions) {
  const [isInstallingDeps, setIsInstallingDeps] = useState(false);

  const installDependencies = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsInstallingDeps(true);
    try {
      await api.install_deps();
      await refetchStatus();
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error installing dependencies', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof ProcessError) {
        logger.error('Process error installing dependencies', {
          error: error.message,
          exitCode: error.exitCode,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error installing dependencies', { error: error.message });
      } else {
        logger.error('Unknown error installing dependencies', { error });
      }
    } finally {
      setIsInstallingDeps(false);
    }
  }, [refetchStatus]);

  return {
    installDependencies,
    isInstallingDeps,
  };
}
