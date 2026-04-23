import { useMemo } from 'react';
import { getAppVersionState, type AppVersionState } from '../utils/appVersionState';
import { useVersions, type UseVersionsResult } from './useVersions';

interface UseSelectedAppVersionsResult {
  appVersions: AppVersionState;
  comfyActiveVersion: string | null;
  comfyInstalledVersions: string[];
  installationProgress: UseVersionsResult['installationProgress'];
  ollamaInstalledVersions: string[];
  torchInstalledVersions: string[];
}

export function useSelectedAppVersions(selectedAppId: string | null): UseSelectedAppVersionsResult {
  const comfyVersions = useVersions({
    appId: 'comfyui',
    trackAvailableVersions: selectedAppId === 'comfyui',
  });
  const ollamaVersions = useVersions({
    appId: 'ollama',
    trackAvailableVersions: selectedAppId === 'ollama',
  });
  const torchVersions = useVersions({
    appId: 'torch',
    trackAvailableVersions: selectedAppId === 'torch',
  });

  const activeVersions = useMemo(() => {
    if (selectedAppId === 'comfyui') return comfyVersions;
    if (selectedAppId === 'ollama') return ollamaVersions;
    if (selectedAppId === 'torch') return torchVersions;
    return comfyVersions;
  }, [selectedAppId, comfyVersions, ollamaVersions, torchVersions]);

  const appVersions = getAppVersionState(selectedAppId, activeVersions);

  return {
    appVersions,
    comfyActiveVersion: comfyVersions.activeVersion,
    comfyInstalledVersions: comfyVersions.installedVersions,
    installationProgress: appVersions.installationProgress,
    ollamaInstalledVersions: ollamaVersions.installedVersions,
    torchInstalledVersions: torchVersions.installedVersions,
  };
}
