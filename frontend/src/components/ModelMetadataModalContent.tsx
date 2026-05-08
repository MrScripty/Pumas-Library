import type {
  BundleComponentManifestEntry,
  InferenceParamSchema,
  ResolvedModelPackageFacts,
} from '../types/api';
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
import { ModelMetadataModalTabs } from './ModelMetadataModalTabs';
import { ModelNotesEditor } from './ModelNotesEditor';
import { ModelRuntimeRouteEditor } from './ModelRuntimeRouteEditor';

interface ModelMetadataModalContentProps {
  activeSource: MetadataSource;
  addingParam: boolean;
  componentManifest: BundleComponentManifestEntry[];
  copiedFieldKey: string | null;
  embeddedFileType: string | null;
  embeddedMetadata: Record<string, unknown> | null;
  executionFacts: ResolvedModelPackageFacts | null;
  executionFactsError: string | null;
  executionFactsLoading: boolean;
  expandedFieldKeys: Set<string>;
  inferenceSettings: InferenceParamSchema[];
  modelId: string;
  modelName: string;
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
  executionFacts,
  executionFactsError,
  executionFactsLoading,
  expandedFieldKeys,
  inferenceSettings,
  modelId,
  modelName,
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

      <ModelMetadataModalTabs
        activeSource={activeSource}
        embeddedFileType={embeddedFileType}
        embeddedMetadata={embeddedMetadata}
        storedMetadata={storedMetadata}
        onActiveSourceChange={onActiveSourceChange}
      />

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
      ) : activeSource === 'execution' ? (
        executionFactsLoading ? (
          <div className="text-center py-4 text-[hsl(var(--text-muted))]">
            Loading execution facts...
          </div>
        ) : executionFactsError ? (
          <div className="text-center py-4 text-[hsl(var(--accent-error))]">
            {executionFactsError}
          </div>
        ) : executionFacts ? (
          <ModelMetadataGrid
            metadata={executionFacts as unknown as Record<string, unknown>}
            isGguf={false}
            sourceKey={activeSource}
            showAllFields={true}
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
        ) : (
          <div className="text-center py-4 text-[hsl(var(--text-muted))]">
            No execution facts available
          </div>
        )
      ) : activeSource === 'runtime' ? (
        <ModelRuntimeRouteEditor
          modelId={modelId}
          modelName={modelName}
          primaryFile={primaryFile}
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
