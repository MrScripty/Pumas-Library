import type { RefObject } from 'react';
import type {
  RuntimeDeviceMode,
  RuntimeProfileConfig,
  RuntimeProfileStatus,
} from '../../types/api-runtime-profiles';
import type { ModelInfo } from '../../types/apps';
import type { ModelServeError, ServedModelStatus } from '../../types/api-serving';
import { ModelServeActions, ModelServeFeedback } from './ModelServeActions';
import { ModelServeForm } from './ModelServeForm';
import { ModelServeHeader } from './ModelServeHeader';
import type { ModelServeControls, ModelServeFormState } from './modelServeHelpers';

interface ModelServeDialogContentProps {
  dialogRef: RefObject<HTMLDivElement | null>;
  profileSelectRef: RefObject<HTMLSelectElement | null>;
  model: ModelInfo;
  isDialogMode: boolean;
  onBack?: () => void;
  onClose: () => void;
  profiles: RuntimeProfileConfig[];
  profileId: string;
  onProfileIdChange: (value: string) => void;
  selectedProfile: RuntimeProfileConfig | undefined;
  selectedStatus: RuntimeProfileStatus | null;
  serveBlockReason: string | null;
  aliasRequired: boolean;
  aliasError: string | null;
  formState: ModelServeFormState;
  setDeviceMode: (value: RuntimeDeviceMode) => void;
  setDeviceId: (value: string) => void;
  setGpuLayers: (value: string) => void;
  setTensorSplit: (value: string) => void;
  setContextSize: (value: string) => void;
  setKeepLoaded: (value: boolean) => void;
  setModelAlias: (value: string) => void;
  controls: ModelServeControls;
  serveError: ModelServeError | null;
  message: string | null;
  isSubmitting: boolean;
  servedStatus: ServedModelStatus | null;
  onServe: () => void;
  onUnload: () => void;
}

export function ModelServeDialogContent({
  dialogRef,
  profileSelectRef,
  model,
  isDialogMode,
  onBack,
  onClose,
  profiles,
  profileId,
  onProfileIdChange,
  selectedProfile,
  selectedStatus,
  serveBlockReason,
  aliasRequired,
  aliasError,
  formState,
  setDeviceMode,
  setDeviceId,
  setGpuLayers,
  setTensorSplit,
  setContextSize,
  setKeepLoaded,
  setModelAlias,
  controls,
  serveError,
  message,
  isSubmitting,
  servedStatus,
  onServe,
  onUnload,
}: ModelServeDialogContentProps) {
  return (
    <div
      ref={dialogRef}
      className={
        isDialogMode
          ? 'w-full max-w-xl rounded-lg border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-primary))] p-4 shadow-2xl'
          : 'space-y-4'
      }
    >
      <ModelServeHeader
        isDialogMode={isDialogMode}
        model={model}
        onBack={onBack}
        onClose={onClose}
      />
      <ModelServeForm
        controls={controls}
        formState={formState}
        model={model}
        onProfileIdChange={onProfileIdChange}
        profileId={profileId}
        profileSelectRef={profileSelectRef}
        profiles={profiles}
        selectedProfile={selectedProfile}
        selectedStatus={selectedStatus}
        serveBlockReason={serveBlockReason}
        aliasRequired={aliasRequired}
        aliasError={aliasError}
        setContextSize={setContextSize}
        setDeviceId={setDeviceId}
        setDeviceMode={setDeviceMode}
        setGpuLayers={setGpuLayers}
        setKeepLoaded={setKeepLoaded}
        setModelAlias={setModelAlias}
        setTensorSplit={setTensorSplit}
      />
      <ModelServeFeedback message={message} serveError={serveError} />
      <ModelServeActions
        isDialogMode={isDialogMode}
        isSubmitting={isSubmitting}
        onClose={onClose}
        onServe={onServe}
        onUnload={onUnload}
        servedStatus={servedStatus}
      />
    </div>
  );
}
