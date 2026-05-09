import { useEffect, useMemo, useRef, useState } from 'react';
import { useRuntimeProfiles } from '../hooks/useRuntimeProfiles';
import type { RuntimeDeviceMode, RuntimeProviderId } from '../types/api-runtime-profiles';
import type { ModelInfo } from '../types/apps';
import { ModelServeDialogContent } from './model-serve/ModelServeDialogContent';
import {
  buildModelServingConfig,
  buildServeBlockReason,
  DEFAULT_LLAMA_CPP_CONTEXT_SIZE,
  getPlacementControls,
  getProfileStateBlockReason,
  isGgufModel,
  isManagedLlamaCppProfile,
  isLlamaCppProfile,
  type ModelServeFormState,
} from './model-serve/modelServeHelpers';
import { useDialogFocusTrap } from './model-serve/useDialogFocusTrap';
import { useModelServingActions } from './model-serve/useModelServingActions';

interface ModelServeDialogProps {
  model: ModelInfo;
  initialProfileId?: string | null;
  providerFilter?: RuntimeProviderId;
  displayMode?: 'dialog' | 'page';
  onBack?: () => void;
  onClose: () => void;
}

export function ModelServeDialog({
  model,
  initialProfileId,
  providerFilter,
  displayMode = 'dialog',
  onBack,
  onClose,
}: ModelServeDialogProps) {
  const runtimeProfiles = useRuntimeProfiles();
  const servingProfiles = useMemo(
    () =>
      providerFilter
        ? runtimeProfiles.profiles.filter((profile) => profile.provider === providerFilter)
        : runtimeProfiles.profiles,
    [providerFilter, runtimeProfiles.profiles]
  );
  const [profileId, setProfileId] = useState('');
  const [deviceMode, setDeviceMode] = useState<RuntimeDeviceMode>('auto');
  const [deviceId, setDeviceId] = useState('');
  const [gpuLayers, setGpuLayers] = useState('');
  const [tensorSplit, setTensorSplit] = useState('');
  const [contextSize, setContextSize] = useState('');
  const [keepLoaded, setKeepLoaded] = useState(true);
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const profileSelectRef = useRef<HTMLSelectElement | null>(null);
  const isDialogMode = displayMode === 'dialog';
  const servingActions = useModelServingActions(model.id);

  useDialogFocusTrap({
    dialogRef,
    initialFocusRef: profileSelectRef,
    isEnabled: isDialogMode,
    onClose,
  });

  useEffect(() => {
    if (profileId) {
      return;
    }

    const selectedInitialProfile = selectInitialServeProfile({
      defaultProfileId: runtimeProfiles.defaultProfileId,
      initialProfileId,
      model,
      profiles: servingProfiles,
      routes: runtimeProfiles.routes,
      statuses: runtimeProfiles.statuses,
    });

    if (!selectedInitialProfile) {
      return;
    }

    setProfileId(selectedInitialProfile.profile_id);
  }, [
    initialProfileId,
    model.id,
    model,
    profileId,
    runtimeProfiles.defaultProfileId,
    runtimeProfiles.routes,
    runtimeProfiles.statuses,
    servingProfiles,
  ]);

  const selectedProfile = servingProfiles.find((profile) => profile.profile_id === profileId);
  const selectedStatus =
    runtimeProfiles.statuses.find((status) => status.profile_id === profileId) ?? null;
  const controls = getPlacementControls(selectedProfile, deviceMode);
  const profileStateBlockReason = getProfileStateBlockReason(selectedProfile, selectedStatus);
  const serveBlockReason = buildServeBlockReason({
    profileError: runtimeProfiles.error,
    isLoading: runtimeProfiles.isLoading,
    servingProfileCount: servingProfiles.length,
    selectedProfile,
    profileStateBlockReason,
    model,
  });

  useEffect(() => {
    if (!selectedProfile) {
      return;
    }

    setDeviceMode(selectedProfile.device.mode);
    setDeviceId(selectedProfile.device.device_id ?? '');
    setGpuLayers(selectedProfile.device.gpu_layers?.toString() ?? '');
    setTensorSplit(selectedProfile.device.tensor_split?.join(',') ?? '');
    setContextSize(isLlamaCppProfile(selectedProfile) ? DEFAULT_LLAMA_CPP_CONTEXT_SIZE : '');
  }, [selectedProfile]);

  const formState: ModelServeFormState = {
    deviceMode,
    deviceId,
    gpuLayers,
    tensorSplit,
    contextSize,
    keepLoaded,
  };
  const buildConfig = () =>
    buildModelServingConfig({
      selectedProfile,
      formState,
      controls,
    });
  const content = (
    <ModelServeDialogContent
      controls={controls}
      dialogRef={dialogRef}
      formState={formState}
      isDialogMode={isDialogMode}
      isSubmitting={servingActions.isSubmitting}
      message={servingActions.message}
      model={model}
      onBack={onBack}
      onClose={onClose}
      onProfileIdChange={setProfileId}
      onServe={() => void servingActions.serveModel(buildConfig())}
      onUnload={() => void servingActions.unloadModel()}
      profileId={profileId}
      profileSelectRef={profileSelectRef}
      profiles={servingProfiles}
      selectedProfile={selectedProfile}
      selectedStatus={selectedStatus}
      serveBlockReason={serveBlockReason}
      serveError={servingActions.serveError}
      servedStatus={servingActions.servedStatus}
      setContextSize={setContextSize}
      setDeviceId={setDeviceId}
      setDeviceMode={setDeviceMode}
      setGpuLayers={setGpuLayers}
      setKeepLoaded={setKeepLoaded}
      setTensorSplit={setTensorSplit}
    />
  );

  if (!isDialogMode) {
    return content;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[hsl(0_0%_0%/0.78)] px-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="model-serve-title"
    >
      {content}
    </div>
  );
}

function selectInitialServeProfile({
  defaultProfileId,
  initialProfileId,
  model,
  profiles,
  routes,
  statuses,
}: {
  defaultProfileId: string | null;
  initialProfileId?: string | null;
  model: ModelInfo;
  profiles: ReturnType<typeof useRuntimeProfiles>['profiles'];
  routes: ReturnType<typeof useRuntimeProfiles>['routes'];
  statuses: ReturnType<typeof useRuntimeProfiles>['statuses'];
}) {
  const explicitProfileId =
    initialProfileId ?? routes.find((route) => route.model_id === model.id)?.profile_id;
  const explicitProfile = profiles.find((profile) => profile.profile_id === explicitProfileId);
  if (explicitProfile) {
    return explicitProfile;
  }

  if (isGgufModel(model)) {
    const runningLlamaProfile = profiles.find((profile) => {
      if (profile.provider !== 'llama_cpp') {
        return false;
      }
      const status = statuses.find((candidate) => candidate.profile_id === profile.profile_id);
      return status?.state === 'running' || status?.state === 'external';
    });
    if (runningLlamaProfile) {
      return runningLlamaProfile;
    }

    const launchableManagedLlamaProfile = profiles.find((profile) =>
      isManagedLlamaCppProfile(profile)
    );
    if (launchableManagedLlamaProfile) {
      return launchableManagedLlamaProfile;
    }
  }

  const defaultProfile = profiles.find((profile) => profile.profile_id === defaultProfileId);
  return defaultProfile ?? profiles.at(0);
}
