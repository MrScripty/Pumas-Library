/**
 * Modal for displaying model metadata (stored and embedded),
 * editing inference settings, and managing user-authored notes.
 *
 * Displays metadata for a library model when ctrl+clicked.
 */

import React, { useEffect, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { modelsAPI } from '../api/models';
import { useModelExecutionFacts } from '../hooks/useModelExecutionFacts';
import type { BundleComponentManifestEntry, InferenceParamSchema } from '../types/api';
import { getStoredNotes, type MetadataSource } from './ModelMetadataFieldConfig';
import { ModelMetadataModalContent } from './ModelMetadataModalContent';
import { ModelMetadataModalFrame } from './ModelMetadataModalFrame';

interface ModelMetadataModalProps {
  modelId: string;
  modelName: string;
  onClose: () => void;
}

export const ModelMetadataModal: React.FC<ModelMetadataModalProps> = ({
  modelId,
  modelName,
  onClose,
}) => {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [storedMetadata, setStoredMetadata] = useState<Record<string, unknown> | null>(null);
  const [embeddedMetadata, setEmbeddedMetadata] = useState<Record<string, unknown> | null>(null);
  const [embeddedFileType, setEmbeddedFileType] = useState<string | null>(null);
  const [primaryFile, setPrimaryFile] = useState<string | null>(null);
  const [componentManifest, setComponentManifest] = useState<BundleComponentManifestEntry[]>([]);
  const [showComponents, setShowComponents] = useState(false);
  const [activeSource, setActiveSource] = useState<MetadataSource>('embedded');
  const [showAllFields, setShowAllFields] = useState(false);
  const [refetching, setRefetching] = useState(false);
  const [refetchError, setRefetchError] = useState<string | null>(null);
  const [expandedFieldKeys, setExpandedFieldKeys] = useState<Set<string>>(new Set());
  const [copiedFieldKey, setCopiedFieldKey] = useState<string | null>(null);
  const { executionFacts, executionFactsError, executionFactsLoading } =
    useModelExecutionFacts(modelId, activeSource);

  // Inference settings state
  const [inferenceSettings, setInferenceSettings] = useState<InferenceParamSchema[]>([]);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [addingParam, setAddingParam] = useState(false);
  const [newParam, setNewParam] = useState({ key: '', label: '', param_type: 'Integer' as InferenceParamSchema['param_type'] });
  const [notesDraft, setNotesDraft] = useState('');
  const [notesPreview, setNotesPreview] = useState(false);
  const [notesSaving, setNotesSaving] = useState(false);
  const [notesSaveError, setNotesSaveError] = useState<string | null>(null);
  const [notesSaveSuccess, setNotesSaveSuccess] = useState(false);

  const serializeFieldValue = (value: unknown): string => {
    const serialized: unknown = JSON.stringify(value, null, 2);
    return typeof serialized === 'string' ? serialized : String(value);
  };

  const handleRefetchFromHF = async () => {
    setRefetching(true);
    setRefetchError(null);
    try {
      const result = await modelsAPI.refetchMetadataFromHF(modelId);
      if (result.success && result.metadata) {
        setStoredMetadata(result.metadata);
        setNotesDraft(getStoredNotes(result.metadata));
        setActiveSource('stored');
      } else {
        setRefetchError(result.error || 'Failed to refetch metadata');
      }
    } catch (e) {
      setRefetchError(e instanceof Error ? e.message : 'Unknown error');
    } finally {
      setRefetching(false);
    }
  };

  useEffect(() => {
    async function fetchMetadata() {
      setLoading(true);
      setError(null);
      try {
        const [metaResult, settingsResult] = await Promise.all([
          modelsAPI.getLibraryModelMetadata(modelId),
          modelsAPI.getInferenceSettings(modelId).catch(() => null),
        ]);

        if (metaResult.success) {
          setStoredMetadata(metaResult.stored_metadata);
          setNotesDraft(getStoredNotes(metaResult.stored_metadata));
          if (metaResult.embedded_metadata) {
            setEmbeddedMetadata(metaResult.embedded_metadata.metadata);
            setEmbeddedFileType(metaResult.embedded_metadata.file_type);
          }
          setPrimaryFile(metaResult.primary_file);
          setComponentManifest(metaResult.component_manifest || []);
        } else {
          setError('Failed to load metadata');
        }

        if (settingsResult !== null && settingsResult.success) {
          setInferenceSettings(settingsResult.inference_settings);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    }
    void fetchMetadata();
  }, [modelId]);

  useEffect(() => {
    setNotesDraft('');
    setNotesPreview(false);
    setNotesSaveError(null);
    setNotesSaveSuccess(false);
  }, [modelId]);

  // ========================================
  // Inference settings handlers
  // ========================================

  const handleParamDefaultChange = (index: number, value: string) => {
    setInferenceSettings((prev) => {
      const next = [...prev];
      const param = next[index];
      if (!param) {
        return prev;
      }
      if (param.param_type === 'Integer') {
        const parsed = parseInt(value, 10);
        param.default = isNaN(parsed) ? value : parsed;
      } else if (param.param_type === 'Number') {
        const parsed = parseFloat(value);
        param.default = isNaN(parsed) ? value : parsed;
      } else if (param.param_type === 'Boolean') {
        param.default = value === 'true';
      } else {
        param.default = value;
      }
      return next;
    });
  };

  const handleRemoveParam = (index: number) => {
    setInferenceSettings((prev) => prev.filter((_, i) => i !== index));
  };

  const handleAddParam = () => {
    if (!newParam.key.trim() || !newParam.label.trim()) return;

    const defaultVal = newParam.param_type === 'Integer' || newParam.param_type === 'Number' ? 0
      : newParam.param_type === 'Boolean' ? false : '';

    setInferenceSettings((prev) => [
      ...prev,
      {
        key: newParam.key.trim(),
        label: newParam.label.trim(),
        param_type: newParam.param_type,
        default: defaultVal,
      },
    ]);
    setNewParam({ key: '', label: '', param_type: 'Integer' });
    setAddingParam(false);
  };

  const handleSaveInferenceSettings = async () => {
    setSaving(true);
    setSaveError(null);
    setSaveSuccess(false);
    try {
      const result = await modelsAPI.updateInferenceSettings(modelId, inferenceSettings);
      if (result.success) {
        setSaveSuccess(true);
        setTimeout(() => setSaveSuccess(false), 2000);
      } else {
        setSaveError('Failed to save settings');
      }
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : 'Unknown error');
    } finally {
      setSaving(false);
    }
  };

  const savedNotes = getStoredNotes(storedMetadata);
  const notesDirty = notesDraft !== savedNotes;

  const handleSaveNotes = async () => {
    setNotesSaving(true);
    setNotesSaveError(null);
    setNotesSaveSuccess(false);
    try {
      const result = await modelsAPI.updateModelNotes(
        modelId,
        notesDraft.trim() === '' ? null : notesDraft
      );
      if (result.success) {
        const nextNotes = result.notes ?? '';
        setNotesDraft(nextNotes);
        setStoredMetadata((prev) => ({
          ...(prev ?? {}),
          notes: result.notes ?? null,
        }));
        setNotesSaveSuccess(true);
        setTimeout(() => setNotesSaveSuccess(false), 2000);
      } else {
        setNotesSaveError(result.error || 'Failed to save notes');
      }
    } catch (e) {
      setNotesSaveError(e instanceof Error ? e.message : 'Unknown error');
    } finally {
      setNotesSaving(false);
    }
  };

  const toggleFieldExpanded = (fieldKey: string) => {
    setExpandedFieldKeys((prev) => {
      const next = new Set(prev);
      if (next.has(fieldKey)) {
        next.delete(fieldKey);
      } else {
        next.add(fieldKey);
      }
      return next;
    });
  };

  const handleCopyFieldValue = async (fieldKey: string, value: unknown) => {
    try {
      const serialized = typeof value === 'string' ? value : serializeFieldValue(value);
      await navigator.clipboard.writeText(serialized);
      setCopiedFieldKey(fieldKey);
      setTimeout(() => {
        setCopiedFieldKey((current) => (current === fieldKey ? null : current));
      }, 1500);
    } catch {
      // Clipboard write failures are non-fatal; leave UI unchanged.
    }
  };

  return (
    <ModelMetadataModalFrame
      isLoading={loading}
      isRefetching={refetching}
      modelName={modelName}
      onClose={onClose}
      onRefetch={() => void handleRefetchFromHF()}
    >
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="w-6 h-6 animate-spin text-[hsl(var(--text-muted))]" />
              <span className="ml-2 text-[hsl(var(--text-muted))]">Loading metadata...</span>
            </div>
          ) : error ? (
            <div className="text-center py-8 text-[hsl(var(--accent-error))]">{error}</div>
          ) : (
            <ModelMetadataModalContent
              activeSource={activeSource}
              addingParam={addingParam}
              componentManifest={componentManifest}
              copiedFieldKey={copiedFieldKey}
              embeddedFileType={embeddedFileType}
              embeddedMetadata={embeddedMetadata}
              executionFacts={executionFacts}
              executionFactsError={executionFactsError}
              executionFactsLoading={executionFactsLoading}
              expandedFieldKeys={expandedFieldKeys}
              inferenceSettings={inferenceSettings}
              newParam={newParam}
              notesDirty={notesDirty}
              notesDraft={notesDraft}
              notesPreview={notesPreview}
              notesSaveError={notesSaveError}
              notesSaveSuccess={notesSaveSuccess}
              notesSaving={notesSaving}
              primaryFile={primaryFile}
              refetchError={refetchError}
              saveError={saveError}
              saveSuccess={saveSuccess}
              saving={saving}
              showAllFields={showAllFields}
              showComponents={showComponents}
              storedMetadata={storedMetadata}
              onActiveSourceChange={setActiveSource}
              onAddParam={handleAddParam}
              onCopyFieldValue={(fieldKey, value) => void handleCopyFieldValue(fieldKey, value)}
              onNewParamChange={setNewParam}
              onNotesDraftChange={setNotesDraft}
              onNotesPreviewChange={setNotesPreview}
              onParamDefaultChange={handleParamDefaultChange}
              onRemoveParam={handleRemoveParam}
              onRevertNotes={() => setNotesDraft(savedNotes)}
              onSaveInferenceSettings={handleSaveInferenceSettings}
              onSaveNotes={handleSaveNotes}
              onSetAddingParam={setAddingParam}
              onToggleComponents={() => setShowComponents((prev) => !prev)}
              onToggleFieldExpanded={toggleFieldExpanded}
              onToggleShowAllFields={() => setShowAllFields((prev) => !prev)}
            />
          )}
    </ModelMetadataModalFrame>
  );
};
