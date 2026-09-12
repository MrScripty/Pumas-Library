import { useEffect, useMemo, useRef, useState } from 'react';
import { useRuntimeProfiles } from '../hooks/useRuntimeProfiles';
import { useServingStatus } from '../hooks/useServingStatus';
import type { RuntimeDeviceMode, RuntimeProviderId } from '../types/api-runtime-profiles';
import type { ModelInfo } from '../types/apps';
import { ModelServeDialogContent } from './model-serve/ModelServeDialogContent';
import { ModalDialog } from './ui';
import {
  buildModelServingConfig,
  buildServeBlockReason,
  defaultContextSizeForProfile,
  getPlacementControls,
  getProfileStateBlockReason,
  profileCanLaunchOnServe,
  type ModelServeFormState,
} from './model-serve/modelServeHelpers';
import { useModelServingActions } from './model-serve/useModelServingActions';
import {
  getRuntimeProviderDescriptor,
  isModelCompatibleWithProvider,
} from '../utils/runtimeProviderDescriptors';

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
  const servingStatus = useServingStatus();
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
  const [modelAlias, setModelAlias] = useState('');
  const draftTargetRef = useRef<{
    modelId: string;
    profileId: string;
    provider: string;
    providerMode: string;
  } | null>(null);
  const profileSelectRef = useRef<HTMLSelectElement | null>(null);
  const isDialogMode = displayMode === 'dialog';
  const selectedProfile = servingProfiles.find((profile) => profile.profile_id === profileId);
  const servingControlObservation = selectedProfile
    ? servingStatus.controlObservation
    : { kind: 'unavailable' as const, message: 'Select a runtime target before serving' };
  const servingActions = useModelServingActions(
    model.id,
    {
      profileId,
      provider: selectedProfile?.provider,
      providerMode: selectedProfile?.provider_mode,
    },
    servingStatus.servedModels,
    servingControlObservation
  );

  useEffect(() => {
    if (profileId) {
      return;
    }

    const selectedInitialProfile = selectInitialServeProfile({
      defaultProfileId: runtimeProfiles.defaultProfileId,
      initialProfileId,
      model,
      profiles: servingProfiles,
      providerFilter,
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
    providerFilter,
    runtimeProfiles.defaultProfileId,
    runtimeProfiles.routes,
    runtimeProfiles.statuses,
    servingProfiles,
  ]);

  const selectedStatus = runtimeProfiles.error
    ? null
    : (runtimeProfiles.statuses.find((status) => status.profile_id === profileId) ?? null);
  const aliasRequired = servingStatus.servedModels.some(
    (servedModel) =>
      servedModel.model_id === model.id &&
      servedModel.load_state !== 'failed' &&
      servedModel.profile_id !== profileId
  );
  const aliasError =
    aliasRequired && !modelAlias.trim()
      ? 'Enter a unique gateway alias before serving this additional instance.'
      : null;
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

    const previousTarget = draftTargetRef.current;
    if (
      previousTarget?.modelId === model.id &&
      previousTarget.profileId === selectedProfile.profile_id &&
      previousTarget.provider === selectedProfile.provider &&
      previousTarget.providerMode === selectedProfile.provider_mode
    ) {
      return;
    }

    draftTargetRef.current = {
      modelId: model.id,
      profileId: selectedProfile.profile_id,
      provider: selectedProfile.provider,
      providerMode: selectedProfile.provider_mode,
    };

    setDeviceMode(selectedProfile.device.mode);
    setDeviceId(selectedProfile.device.device_id ?? '');
    setGpuLayers(selectedProfile.device.gpu_layers?.toString() ?? '');
    setTensorSplit(selectedProfile.device.tensor_split?.join(',') ?? '');
    setContextSize(defaultContextSizeForProfile(selectedProfile));
    setKeepLoaded(true);
    setModelAlias('');
  }, [model.id, selectedProfile]);

  const formState: ModelServeFormState = {
    deviceMode,
    deviceId,
    gpuLayers,
    tensorSplit,
    contextSize,
    keepLoaded,
    modelAlias,
  };
  const buildConfig = () =>
    buildModelServingConfig({
      selectedProfile,
      formState,
      controls,
    });
  const content = (
    <ModelServeDialogContent
      actionPhase={servingActions.actionPhase}
      controlObservation={servingControlObservation}
      controls={controls}
      formState={formState}
      isDialogMode={isDialogMode}
      isLoading={servingActions.isLoading}
      isUnavailable={servingActions.isUnavailable}
      message={servingActions.message}
      model={model}
      aliasRequired={aliasRequired}
      aliasError={aliasError}
      onBack={onBack}
      onClose={onClose}
      onProfileIdChange={setProfileId}
      onServe={async () => {
        if (aliasError) {
          return;
        }
        try {
          await servingActions.serveModel(buildConfig());
        } finally {
          await Promise.all([
            runtimeProfiles.refreshRuntimeProfiles(),
            servingStatus.refreshServingStatus(),
          ]);
        }
      }}
      onUnload={async () => {
        try {
          await servingActions.unloadModel();
        } finally {
          await Promise.all([
            runtimeProfiles.refreshRuntimeProfiles(),
            servingStatus.refreshServingStatus(),
          ]);
        }
      }}
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
      setModelAlias={setModelAlias}
      setTensorSplit={setTensorSplit}
    />
  );

  if (!isDialogMode) {
    return content;
  }

  return (
    <ModalDialog
      ariaLabel={`Serve ${model.name}`}
      backdropClassName="bg-[hsl(0_0%_0%/0.78)] backdrop-blur-sm"
      contentClassName="w-full max-w-xl"
      initialFocusRef={profileSelectRef}
      isOpen={true}
      onClose={onClose}
      overlayClassName="fixed inset-0 z-50 flex items-center justify-center px-4"
      shouldCloseOnBackdrop={false}
    >
      {content}
    </ModalDialog>
  );
}

function selectInitialServeProfile({
  defaultProfileId,
  initialProfileId,
  model,
  profiles,
  providerFilter,
  routes,
  statuses,
}: {
  defaultProfileId: string | null;
  initialProfileId?: string | null;
  model: ModelInfo;
  profiles: ReturnType<typeof useRuntimeProfiles>['profiles'];
  providerFilter?: RuntimeProviderId;
  routes: ReturnType<typeof useRuntimeProfiles>['routes'];
  statuses: ReturnType<typeof useRuntimeProfiles>['statuses'];
}) {
  const explicitProfileId =
    initialProfileId ??
    routes.find(
      (route) =>
        route.model_id === model.id && (!providerFilter || route.provider === providerFilter)
    )?.profile_id;
  const explicitProfile = profiles.find((profile) => profile.profile_id === explicitProfileId);
  if (explicitProfile) {
    return explicitProfile;
  }
  if (
    providerFilter &&
    getRuntimeProviderDescriptor(providerFilter).requiresSavedRouteForImplicitServe
  ) {
    return undefined;
  }

  const launchOnServeProfiles = profiles.filter((profile) => {
    const descriptor = getRuntimeProviderDescriptor(profile.provider);
    return descriptor.canLaunchOnServe && isModelCompatibleWithProvider(model, profile.provider);
  });
  if (launchOnServeProfiles.length > 0) {
    const runningLaunchOnServeProfile = launchOnServeProfiles.find((profile) => {
      const status = statuses.find((candidate) => candidate.profile_id === profile.profile_id);
      return status?.state === 'running' || status?.state === 'external';
    });
    if (runningLaunchOnServeProfile) {
      return runningLaunchOnServeProfile;
    }

    const launchableManagedProfile = launchOnServeProfiles.find((profile) =>
      profileCanLaunchOnServe(profile)
    );
    if (launchableManagedProfile) {
      return launchableManagedProfile;
    }
  }

  const defaultProfile = profiles.find((profile) => profile.profile_id === defaultProfileId);
  return defaultProfile ?? profiles.at(0);
}
