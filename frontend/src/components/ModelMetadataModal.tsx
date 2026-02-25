/**
 * Modal for displaying model metadata (stored and embedded)
 * and editing inference settings.
 *
 * Displays metadata for a library model when ctrl+clicked.
 */

import React, { useEffect, useState } from 'react';
import { X, FileText, Database, ChevronDown, ChevronUp, ExternalLink, Loader2, RefreshCw, Settings, Plus, Trash2 } from 'lucide-react';
import { modelsAPI } from '../api/models';
import type { InferenceParamSchema } from '../types/api';

interface ModelMetadataModalProps {
  modelId: string;
  modelName: string;
  onClose: () => void;
}

/** Priority GGUF fields to show in compact view */
const PRIORITY_GGUF_FIELDS = [
  'general.name',
  'general.basename',
  'general.architecture',
  'general.size_label',
  'general.finetune',
  'general.tags',
  'general.license',
  'general.quantized_by',
];

/** Patterns for architecture-specific priority fields */
const PRIORITY_ARCH_PATTERNS = ['.context_length', '.embedding_length'];

/** Field pairs: display field -> URL field */
const LINKED_GGUF_FIELDS: Record<string, string> = {
  'general.basename': 'general.base_model.0.repo_url',
  'general.license': 'general.license.link',
  'general.quantized_by': 'general.repo_url',
};

/** URL fields that are used as link targets */
const URL_TARGET_FIELDS = new Set(Object.values(LINKED_GGUF_FIELDS));

/** Fields to always hide unless "Show all" */
const HIDDEN_GGUF_FIELDS = new Set([
  'tokenizer.chat_template',
  'tokenizer.ggml.merges',
  'tokenizer.ggml.tokens',
  'tokenizer.ggml.token_type',
  'tokenizer.ggml.scores',
]);

/** Format field name for display */
function formatFieldName(key: string): string {
  return key
    .replace(/^general\./, '')
    .replace(/\./g, ' ')
    .replace(/_/g, ' ')
    .split(' ')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

/** Format metadata value for display */
function formatMetadataValue(key: string, value: unknown): string {
  if (value == null) return '';
  if (Array.isArray(value)) return value.join(', ');
  if (key === 'match_confidence' && typeof value === 'number') {
    return `${Math.round(value * 100)}%`;
  }
  if (typeof value === 'boolean') return value ? 'Yes' : 'No';
  return String(value);
}

/** Check if a GGUF field is a priority field */
function isPriorityGgufField(key: string): boolean {
  const lowerKey = key.toLowerCase();
  if (PRIORITY_GGUF_FIELDS.some((f) => lowerKey === f)) return true;
  if (PRIORITY_ARCH_PATTERNS.some((p) => lowerKey.endsWith(p))) return true;
  return false;
}

/** Check if a GGUF field should be hidden by default */
function isHiddenGgufField(key: string, value: unknown): boolean {
  const lowerKey = key.toLowerCase();
  if (HIDDEN_GGUF_FIELDS.has(lowerKey)) return true;
  if (URL_TARGET_FIELDS.has(lowerKey)) return true;
  const strValue = String(value ?? '');
  if (strValue.length > 500) return true;
  return false;
}

type MetadataSource = 'stored' | 'embedded' | 'inference';

const PARAM_TYPE_OPTIONS: { value: InferenceParamSchema['param_type']; label: string }[] = [
  { value: 'Integer', label: 'Integer' },
  { value: 'Number', label: 'Number' },
  { value: 'String', label: 'String' },
  { value: 'Boolean', label: 'Boolean' },
];

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
  const [activeSource, setActiveSource] = useState<MetadataSource>('embedded');
  const [showAllFields, setShowAllFields] = useState(false);
  const [refetching, setRefetching] = useState(false);
  const [refetchError, setRefetchError] = useState<string | null>(null);

  // Inference settings state
  const [inferenceSettings, setInferenceSettings] = useState<InferenceParamSchema[]>([]);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [addingParam, setAddingParam] = useState(false);
  const [newParam, setNewParam] = useState({ key: '', label: '', param_type: 'Integer' as InferenceParamSchema['param_type'] });

  const handleRefetchFromHF = async () => {
    setRefetching(true);
    setRefetchError(null);
    try {
      const result = await modelsAPI.refetchMetadataFromHF(modelId);
      if (result.success && result.metadata) {
        setStoredMetadata(result.metadata);
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
          if (metaResult.embedded_metadata) {
            setEmbeddedMetadata(metaResult.embedded_metadata.metadata);
            setEmbeddedFileType(metaResult.embedded_metadata.file_type);
          }
          setPrimaryFile(metaResult.primary_file);
        } else {
          setError('Failed to load metadata');
        }

        if (settingsResult?.success) {
          setInferenceSettings(settingsResult.inference_settings || []);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    }
    void fetchMetadata();
  }, [modelId]);

  // Handle escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  // ========================================
  // Inference settings handlers
  // ========================================

  const handleParamDefaultChange = (index: number, value: string) => {
    setInferenceSettings((prev) => {
      const next = [...prev];
      const param = next[index];
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

  // ========================================
  // Metadata grid renderer
  // ========================================

  const renderMetadataGrid = (metadata: Record<string, unknown>, isGguf: boolean) => {
    const entries = Object.entries(metadata);

    // Sort and filter entries
    let displayEntries = entries;
    if (isGguf) {
      // Filter and sort GGUF entries
      const priorityEntries = entries.filter(([k]) => isPriorityGgufField(k));
      const otherEntries = entries.filter(
        ([k, v]) => !isPriorityGgufField(k) && !isHiddenGgufField(k, v)
      );
      const hiddenEntries = entries.filter(([k, v]) => isHiddenGgufField(k, v));

      displayEntries = showAllFields
        ? [...priorityEntries, ...otherEntries, ...hiddenEntries]
        : [...priorityEntries, ...otherEntries.slice(0, 5)];

      const hiddenCount = hiddenEntries.length + (showAllFields ? 0 : Math.max(0, otherEntries.length - 5));

      return (
        <div className="space-y-2">
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm max-h-80 overflow-y-auto">
            {displayEntries.map(([key, value]) => {
              const linkedUrl = LINKED_GGUF_FIELDS[key.toLowerCase()];
              const urlValue = linkedUrl ? (metadata[linkedUrl] as string) : null;

              return (
                <React.Fragment key={key}>
                  <span className="text-[hsl(var(--text-muted))] truncate" title={key}>
                    {formatFieldName(key)}
                  </span>
                  <span className="text-[hsl(var(--text-secondary))] truncate" title={formatMetadataValue(key, value)}>
                    {urlValue ? (
                      <a
                        href={urlValue}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-[hsl(var(--accent-link))] hover:underline inline-flex items-center gap-1"
                      >
                        {formatMetadataValue(key, value)}
                        <ExternalLink className="w-3 h-3" />
                      </a>
                    ) : (
                      formatMetadataValue(key, value)
                    )}
                  </span>
                </React.Fragment>
              );
            })}
          </div>
          {hiddenCount > 0 && (
            <button
              onClick={() => setShowAllFields(!showAllFields)}
              className="text-xs text-[hsl(var(--text-muted))] hover:text-[hsl(var(--text-primary))] flex items-center gap-1"
            >
              {showAllFields ? (
                <>
                  <ChevronUp className="w-3 h-3" /> Show less
                </>
              ) : (
                <>
                  <ChevronDown className="w-3 h-3" /> Show {hiddenCount} more fields
                </>
              )}
            </button>
          )}
        </div>
      );
    }

    // Non-GGUF (stored metadata) - simpler display
    return (
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm max-h-80 overflow-y-auto">
        {displayEntries.map(([key, value]) => (
          <React.Fragment key={key}>
            <span className="text-[hsl(var(--text-muted))] truncate" title={key}>
              {formatFieldName(key)}
            </span>
            <span className="text-[hsl(var(--text-secondary))] truncate" title={formatMetadataValue(key, value)}>
              {formatMetadataValue(key, value)}
            </span>
          </React.Fragment>
        ))}
      </div>
    );
  };

  // ========================================
  // Inference settings panel renderer
  // ========================================

  const renderInferenceSettings = () => (
    <div className="space-y-4 max-h-80 overflow-y-auto">
      {inferenceSettings.length === 0 ? (
        <div className="text-center py-4 text-[hsl(var(--text-muted))] text-sm">
          No inference settings configured for this model.
        </div>
      ) : (
        <div className="space-y-2">
          {inferenceSettings.map((param, index) => (
            <div key={param.key} className="flex items-center gap-2">
              <div className="flex-1 min-w-0">
                <label
                  className="block text-xs text-[hsl(var(--text-muted))] truncate"
                  title={param.description || param.key}
                >
                  {param.label}
                  <span className="ml-1 opacity-50">({param.param_type})</span>
                </label>
                {param.param_type === 'Boolean' ? (
                  <select
                    value={String(param.default)}
                    onChange={(e) => handleParamDefaultChange(index, e.target.value)}
                    className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                  >
                    <option value="true">true</option>
                    <option value="false">false</option>
                  </select>
                ) : param.param_type === 'String' && param.constraints?.allowed_values ? (
                  <select
                    value={String(param.default ?? '')}
                    onChange={(e) => handleParamDefaultChange(index, e.target.value)}
                    className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                  >
                    {param.constraints.allowed_values.map((v) => (
                      <option key={String(v)} value={String(v)}>
                        {String(v)}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    type={param.param_type === 'String' ? 'text' : 'number'}
                    value={param.default == null ? '' : String(param.default)}
                    onChange={(e) => handleParamDefaultChange(index, e.target.value)}
                    placeholder={param.description || param.key}
                    min={param.constraints?.min ?? undefined}
                    max={param.constraints?.max ?? undefined}
                    step={param.param_type === 'Integer' ? 1 : 'any'}
                    className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                  />
                )}
              </div>
              <button
                onClick={() => handleRemoveParam(index)}
                className="p-1 mt-4 text-[hsl(var(--text-muted))] hover:text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error)/0.1)] rounded"
                title="Remove parameter"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Add parameter form */}
      {addingParam ? (
        <div className="space-y-2 p-3 bg-[hsl(var(--surface-high)/0.5)] rounded border border-[hsl(var(--border-default))]">
          <div className="grid grid-cols-3 gap-2">
            <input
              type="text"
              value={newParam.key}
              onChange={(e) => setNewParam((p) => ({ ...p, key: e.target.value }))}
              placeholder="key"
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            />
            <input
              type="text"
              value={newParam.label}
              onChange={(e) => setNewParam((p) => ({ ...p, label: e.target.value }))}
              placeholder="Label"
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            />
            <select
              value={newParam.param_type}
              onChange={(e) => setNewParam((p) => ({ ...p, param_type: e.target.value as InferenceParamSchema['param_type'] }))}
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            >
              {PARAM_TYPE_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
          <div className="flex gap-2">
            <button
              onClick={handleAddParam}
              disabled={!newParam.key.trim() || !newParam.label.trim()}
              className="px-3 py-1 text-xs bg-[hsl(var(--launcher-accent-primary))] text-white rounded hover:opacity-90 disabled:opacity-40"
            >
              Add
            </button>
            <button
              onClick={() => setAddingParam(false)}
              className="px-3 py-1 text-xs bg-[hsl(var(--surface-high))] text-[hsl(var(--text-secondary))] rounded hover:bg-[hsl(var(--surface-mid))]"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setAddingParam(true)}
          className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))] hover:text-[hsl(var(--text-primary))]"
        >
          <Plus className="w-3 h-3" /> Add Parameter
        </button>
      )}

      {/* Save button and status */}
      <div className="flex items-center gap-3 pt-2 border-t border-[hsl(var(--border-default))]">
        <button
          onClick={handleSaveInferenceSettings}
          disabled={saving}
          className="px-4 py-1.5 text-sm bg-[hsl(var(--launcher-accent-primary))] text-white rounded hover:opacity-90 disabled:opacity-50"
        >
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
        {saveSuccess && (
          <span className="text-xs text-[hsl(var(--accent-success))]">Saved</span>
        )}
        {saveError && (
          <span className="text-xs text-[hsl(var(--accent-error))]">{saveError}</span>
        )}
      </div>
    </div>
  );

  return (
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- modal backdrop dismiss
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- prevent backdrop dismiss propagation */}
      <div
        className="bg-[hsl(var(--surface-overlay)/0.95)] border border-[hsl(var(--border-default))] backdrop-blur-md rounded-lg shadow-lg w-full max-w-2xl max-h-[80vh] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-[hsl(var(--border-default))]">
          <h2 className="text-lg font-semibold truncate text-[hsl(var(--text-primary))]">{modelName}</h2>
          <div className="flex items-center gap-1">
            <button
              onClick={handleRefetchFromHF}
              disabled={refetching || loading}
              className="p-1 hover:bg-[hsl(var(--surface-mid))] rounded text-[hsl(var(--text-secondary))] disabled:opacity-40"
              aria-label="Refetch metadata from HuggingFace"
              title="Refetch metadata from HuggingFace"
            >
              <RefreshCw className={`w-4 h-4 ${refetching ? 'animate-spin' : ''}`} />
            </button>
            <button
              onClick={onClose}
              className="p-1 hover:bg-[hsl(var(--surface-mid))] rounded text-[hsl(var(--text-secondary))]"
              aria-label="Close"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="p-4 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="w-6 h-6 animate-spin text-[hsl(var(--text-muted))]" />
              <span className="ml-2 text-[hsl(var(--text-muted))]">Loading metadata...</span>
            </div>
          ) : error ? (
            <div className="text-center py-8 text-[hsl(var(--accent-error))]">{error}</div>
          ) : (
            <div className="space-y-4">
              {/* Refetch error */}
              {refetchError && (
                <div className="text-xs text-[hsl(var(--accent-error))] bg-[hsl(var(--accent-error)/0.1)] px-3 py-1.5 rounded">
                  {refetchError}
                </div>
              )}

              {/* Source toggle */}
              <div className="flex gap-2">
                <button
                  onClick={() => setActiveSource('embedded')}
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
                  onClick={() => setActiveSource('stored')}
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
                  onClick={() => setActiveSource('inference')}
                  className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
                    activeSource === 'inference'
                      ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
                      : 'bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))] text-[hsl(var(--text-secondary))]'
                  }`}
                >
                  <Settings className="w-4 h-4" />
                  Inference
                </button>
              </div>

              {/* Content for active tab */}
              {activeSource === 'embedded' && embeddedMetadata ? (
                renderMetadataGrid(embeddedMetadata, embeddedFileType === 'gguf')
              ) : activeSource === 'stored' && storedMetadata ? (
                renderMetadataGrid(storedMetadata, false)
              ) : activeSource === 'inference' ? (
                renderInferenceSettings()
              ) : (
                <div className="text-center py-4 text-[hsl(var(--text-muted))]">
                  No {activeSource} metadata available
                </div>
              )}

              {/* Primary file path */}
              {primaryFile && (
                <div className="pt-2 border-t border-[hsl(var(--border-default))]">
                  <span className="text-xs text-[hsl(var(--text-muted))]">File: </span>
                  <span className="text-xs font-mono truncate text-[hsl(var(--text-secondary))]">{primaryFile}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
