import { Database, FileText, PencilLine, Settings } from 'lucide-react';
import type { BundleComponentManifestEntry, InferenceParamSchema } from '../types/api';
import {
  formatFieldName,
  formatMetadataValue,
  isHiddenGgufField,
  isPriorityGgufField,
  LINKED_GGUF_FIELDS,
  type MetadataSource,
} from './ModelMetadataFieldConfig';
import { ModelBundleManifestPanel } from './ModelBundleManifestPanel';
import { ModelInferenceSettingsEditor } from './ModelInferenceSettingsEditor';
import { ModelMetadataGrid } from './ModelMetadataGrid';
import { ModelNotesEditor } from './ModelNotesEditor';

interface ModelMetadataModalContentProps {
  activeSource: MetadataSource;
  addingParam: boolean;
  componentManifest: BundleComponentManifestEntry[];
  copiedFieldKey: string | null;
  embeddedFileType: string | null;
  embeddedMetadata: Record<string, unknown> | null;
  expandedFieldKeys: Set<string>;
  inferenceSettings: InferenceParamSchema[];
  newParam: {
    key: string;
    label: string;
    param_type: InferenceParamSchema['param_type'];
  };
  notesDirty: boolean;
  notesDraft: string;
  notesPreview: boolean;
  notesSaveError: string | null;
  notesSaveSuccess: boolean;
  notesSaving: boolean;
  primaryFile: string | null;
  refetchError: string | null;
  saveError: string | null;
  saveSuccess: boolean;
  saving: boolean;
  showAllFields: boolean;
  showComponents: boolean;
  storedMetadata: Record<string, unknown> | null;
  onActiveSourceChange: (source: MetadataSource) => void;
  onAddParam: () => void;
  onCopyFieldValue: (fieldKey: string, value: unknown) => void;
  onNewParamChange: (
    updater: (
      current: {
        key: string;
        label: string;
        param_type: InferenceParamSchema['param_type'];
      }
    ) => {
      key: string;
      label: string;
      param_type: InferenceParamSchema['param_type'];
    }
  ) => void;
  onNotesDraftChange: (next: string) => void;
  onNotesPreviewChange: (next: boolean) => void;
  onParamDefaultChange: (index: number, value: string) => void;
  onRemoveParam: (index: number) => void;
  onRevertNotes: () => void;
  onSaveInferenceSettings: () => void;
  onSaveNotes: () => void;
  onSetAddingParam: (next: boolean) => void;
  onToggleComponents: () => void;
  onToggleFieldExpanded: (fieldKey: string) => void;
  onToggleShowAllFields: () => void;
}

export function ModelMetadataModalContent({
  activeSource,
  addingParam,
  componentManifest,
  copiedFieldKey,
  embeddedFileType,
  embeddedMetadata,
  expandedFieldKeys,
  inferenceSettings,
  newParam,
  notesDirty,
  notesDraft,
  notesPreview,
  notesSaveError,
  notesSaveSuccess,
  notesSaving,
  primaryFile,
  refetchError,
  saveError,
  saveSuccess,
  saving,
  showAllFields,
  showComponents,
  storedMetadata,
  onActiveSourceChange,
  onAddParam,
  onCopyFieldValue,
  onNewParamChange,
  onNotesDraftChange,
  onNotesPreviewChange,
  onParamDefaultChange,
  onRemoveParam,
  onRevertNotes,
  onSaveInferenceSettings,
  onSaveNotes,
  onSetAddingParam,
  onToggleComponents,
  onToggleFieldExpanded,
  onToggleShowAllFields,
}: ModelMetadataModalContentProps) {
  return (
    <div className="space-y-4">
      {refetchError && (
        <div className="text-xs text-[hsl(var(--accent-error))] bg-[hsl(var(--accent-error)/0.1)] px-3 py-1.5 rounded">
          {refetchError}
        </div>
      )}

      <div className="flex gap-2">
        <button
          onClick={() => onActiveSourceChange('embedded')}
          className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
            activeSource === 'embedded'
              ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
              : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
          }`}
          disabled={!embeddedMetadata}
        >
          <FileText className="w-4 h-4" />
          Embedded ({embeddedFileType || 'N/A'})
        </button>
        <button
          onClick={() => onActiveSourceChange('stored')}
          className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
            activeSource === 'stored'
              ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
              : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
          }`}
          disabled={!storedMetadata}
        >
          <Database className="w-4 h-4" />
          Stored
        </button>
        <button
          onClick={() => onActiveSourceChange('inference')}
          className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
            activeSource === 'inference'
              ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
              : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
          }`}
        >
          <Settings className="w-4 h-4" />
          Inference
        </button>
        <button
          onClick={() => onActiveSourceChange('notes')}
          className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
            activeSource === 'notes'
              ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
              : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
          }`}
        >
          <PencilLine className="w-4 h-4" />
          Notes
        </button>
      </div>

      {activeSource === 'embedded' && embeddedMetadata ? (
        <ModelMetadataGrid
          metadata={embeddedMetadata}
          isGguf={embeddedFileType === 'gguf'}
          sourceKey={activeSource}
          showAllFields={showAllFields}
          expandedFieldKeys={expandedFieldKeys}
          copiedFieldKey={copiedFieldKey}
          onToggleShowAllFields={onToggleShowAllFields}
          onToggleFieldExpanded={onToggleFieldExpanded}
          onCopyFieldValue={onCopyFieldValue}
          formatFieldName={formatFieldName}
          formatMetadataValue={formatMetadataValue}
          isPriorityGgufField={isPriorityGgufField}
          isHiddenGgufField={isHiddenGgufField}
          linkedGgufFields={LINKED_GGUF_FIELDS}
        />
      ) : activeSource === 'stored' && storedMetadata ? (
        <ModelMetadataGrid
          metadata={storedMetadata}
          isGguf={false}
          sourceKey={activeSource}
          showAllFields={showAllFields}
          expandedFieldKeys={expandedFieldKeys}
          copiedFieldKey={copiedFieldKey}
          onToggleShowAllFields={onToggleShowAllFields}
          onToggleFieldExpanded={onToggleFieldExpanded}
          onCopyFieldValue={onCopyFieldValue}
          formatFieldName={formatFieldName}
          formatMetadataValue={formatMetadataValue}
          isPriorityGgufField={isPriorityGgufField}
          isHiddenGgufField={isHiddenGgufField}
          linkedGgufFields={LINKED_GGUF_FIELDS}
        />
      ) : activeSource === 'inference' ? (
        <ModelInferenceSettingsEditor
          addingParam={addingParam}
          inferenceSettings={inferenceSettings}
          newParam={newParam}
          saveError={saveError}
          saveSuccess={saveSuccess}
          saving={saving}
          onAddParam={onAddParam}
          onNewParamChange={onNewParamChange}
          onParamDefaultChange={onParamDefaultChange}
          onRemoveParam={onRemoveParam}
          onSave={onSaveInferenceSettings}
          onSetAddingParam={onSetAddingParam}
        />
      ) : activeSource === 'notes' ? (
        <ModelNotesEditor
          notesDraft={notesDraft}
          notesPreview={notesPreview}
          notesSaving={notesSaving}
          notesDirty={notesDirty}
          notesSaveError={notesSaveError}
          notesSaveSuccess={notesSaveSuccess}
          onNotesDraftChange={onNotesDraftChange}
          onNotesPreviewChange={onNotesPreviewChange}
          onSaveNotes={onSaveNotes}
          onRevertNotes={onRevertNotes}
        />
      ) : (
        <div className="text-center py-4 text-[hsl(var(--text-muted))]">
          No {activeSource} metadata available
        </div>
      )}

      <ModelBundleManifestPanel
        componentManifest={componentManifest}
        showComponents={showComponents}
        onToggle={onToggleComponents}
      />

      {primaryFile && (
        <div className="pt-2 border-t border-[hsl(var(--border-default))]">
          <span className="text-xs text-[hsl(var(--text-muted))]">File: </span>
          <span className="text-xs font-mono truncate text-[hsl(var(--text-secondary))]">{primaryFile}</span>
        </div>
      )}
    </div>
  );
}
