import { useState } from 'react';
import { getElectronAPI } from '../../../api/adapter';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type { ModelCategory } from '../../../types/apps';
import type { ServedModelStatus } from '../../../types/api-serving';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import { getRuntimeProviderDescriptor } from '../../../utils/runtimeProviderDescriptors';
import { ModelMetadataModal } from '../../ModelMetadataModal';
import { ModelServeDialog } from '../../ModelServeDialog';
import { clearModelRuntimeRoute, saveModelRuntimeRoute } from '../../model-serve/runtimeRouteMutations';
import { serveModelWithValidation } from '../../model-serve/useModelServingActions';
import { buildLlamaCppModelRows, type LlamaCppModelRowViewModel } from './llamaCppLibraryViewModels';
import { LlamaCppModelLibraryList } from './LlamaCppModelLibraryList';
import {
  buildQuickServeConfig,
  formatQuickServeError,
  requiresAliasBeforeQuickServe,
} from './llamaCppQuickServe';

const LLAMA_CPP_PROVIDER = getRuntimeProviderDescriptor('llama_cpp').id;

interface ServingTarget {
  row: LlamaCppModelRowViewModel;
  profileId: string;
}

export interface LlamaCppModelLibrarySectionProps {
  excludedModels: Set<string>;
  modelGroups: ModelCategory[];
  servedModels?: ServedModelStatus[];
  starredModels: Set<string>;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

export function LlamaCppModelLibrarySection({
  excludedModels,
  modelGroups,
  servedModels = [],
  starredModels,
  onToggleLink,
  onToggleStar,
}: LlamaCppModelLibrarySectionProps) {
  const { profiles, routes, refreshRuntimeProfiles } = useRuntimeProfiles();
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const [servingTarget, setServingTarget] = useState<ServingTarget | null>(null);
  const [quickServeModelId, setQuickServeModelId] = useState<string | null>(null);
  const [quickServeFeedback, setQuickServeFeedback] = useState<{
    kind: 'error' | 'success';
    message: string;
    modelId: string;
  } | null>(null);
  const [savingRouteModelId, setSavingRouteModelId] = useState<string | null>(null);
  const [routeError, setRouteError] = useState<string | null>(null);
  const providerProfiles = profiles.filter((profile) => profile.provider === LLAMA_CPP_PROVIDER);
  const rows = buildLlamaCppModelRows({
    modelGroups,
    profiles,
    routes,
    servedStatuses: servedModels,
  });

  const persistRouteSelection = async (modelId: string, profileId: string): Promise<boolean> => {
    setSavingRouteModelId(modelId);
    setRouteError(null);
    try {
      if (profileId) {
        await saveModelRuntimeRoute({
          provider: LLAMA_CPP_PROVIDER,
          modelId,
          profileId,
          autoLoad: true,
        });
      } else {
        await clearModelRuntimeRoute(LLAMA_CPP_PROVIDER, modelId);
      }
      await refreshRuntimeProfiles();
      return true;
    } catch (caught) {
      setRouteError(caught instanceof Error ? caught.message : 'Failed to save runtime route');
      return false;
    } finally {
      setSavingRouteModelId(null);
    }
  };

  const handleSaveRoute = async (modelId: string, profileId: string) => {
    await persistRouteSelection(modelId, profileId);
  };

  const handleOpenServeOptions = async (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => {
    if (shouldPersistRoute) {
      const saved = await persistRouteSelection(row.model.id, profile.profile_id);
      if (!saved) {
        return;
      }
    }
    setServingTarget({ row, profileId: profile.profile_id });
  };

  const handleQuickServe = async (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => {
    if (!profile) {
      setQuickServeFeedback({
        kind: 'error',
        message: 'Select a llama.cpp profile before serving this model.',
        modelId: row.model.id,
      });
      return;
    }

    if (shouldPersistRoute) {
      const saved = await persistRouteSelection(row.model.id, profile.profile_id);
      if (!saved) {
        return;
      }
    }

    if (requiresAliasBeforeQuickServe(row, profile)) {
      setServingTarget({ row, profileId: profile.profile_id });
      return;
    }

    const api = getElectronAPI();
    if (!api?.validate_model_serving_config || !api.serve_model) {
      setQuickServeFeedback({
        kind: 'error',
        message: 'Serving API is not available in this app session.',
        modelId: row.model.id,
      });
      return;
    }

    setQuickServeModelId(row.model.id);
    setQuickServeFeedback(null);
    try {
      const result = await serveModelWithValidation({
        api,
        config: buildQuickServeConfig(profile),
        modelId: row.model.id,
      });

      if (result.kind === 'validation_failed' && result.error.code === 'duplicate_model_alias') {
        setServingTarget({ row, profileId: profile.profile_id });
        return;
      }
      if (result.kind === 'loaded') {
        setQuickServeFeedback({
          kind: 'success',
          message: 'Loaded',
          modelId: row.model.id,
        });
        return;
      }
      if (result.kind === 'validation_failed' || result.kind === 'load_failed') {
        setQuickServeFeedback({
          kind: 'error',
          message:
            formatQuickServeError(result.error) ??
            'The selected llama.cpp profile cannot serve this model.',
          modelId: row.model.id,
        });
        return;
      }
      setQuickServeFeedback({
        kind: 'error',
        message: result.message,
        modelId: row.model.id,
      });
    } catch (caught) {
      setQuickServeFeedback({
        kind: 'error',
        message: caught instanceof Error ? caught.message : 'Serving request failed',
        modelId: row.model.id,
      });
    } finally {
      setQuickServeModelId(null);
    }
  };

  if (servingTarget) {
    return (
      <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
        <ModelServeDialog
          model={servingTarget.row.model}
          displayMode="page"
          initialProfileId={servingTarget.profileId}
          providerFilter="llama_cpp"
          onBack={() => setServingTarget(null)}
          onClose={() => setServingTarget(null)}
        />
      </section>
    );
  }

  return (
    <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
      <LlamaCppModelLibraryList
        excludedModels={excludedModels}
        providerProfiles={providerProfiles}
        quickServeFeedback={quickServeFeedback}
        quickServeModelId={quickServeModelId}
        routeError={routeError}
        rows={rows}
        savingRouteModelId={savingRouteModelId}
        starredModels={starredModels}
        onOpenMetadata={(modelId, modelName) => {
          setMetadataModal({ modelId, modelName });
        }}
        onOpenServeOptions={(selectedRow, profile, shouldPersistRoute) => {
          void handleOpenServeOptions(selectedRow, profile, shouldPersistRoute);
        }}
        onQuickServe={(selectedRow, profile, shouldPersistRoute) => {
          void handleQuickServe(selectedRow, profile, shouldPersistRoute);
        }}
        onSaveRoute={(modelId, profileId) => {
          void handleSaveRoute(modelId, profileId);
        }}
        onToggleLink={onToggleLink}
        onToggleStar={onToggleStar}
      />
      {metadataModal && (
        <ModelMetadataModal
          modelId={metadataModal.modelId}
          modelName={metadataModal.modelName}
          onClose={() => setMetadataModal(null)}
        />
      )}
    </section>
  );
}
