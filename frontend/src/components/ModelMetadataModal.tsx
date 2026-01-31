/**
 * Modal for displaying model metadata (stored and embedded)
 *
 * Displays metadata for a library model when ctrl+clicked.
 */

import React, { useEffect, useState } from 'react';
import { X, FileText, Database, ChevronDown, ChevronUp, ExternalLink, Loader2 } from 'lucide-react';
import { modelsAPI } from '../api/models';

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

type MetadataSource = 'stored' | 'embedded';

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

  useEffect(() => {
    async function fetchMetadata() {
      setLoading(true);
      setError(null);
      try {
        const result = await modelsAPI.getLibraryModelMetadata(modelId);
        if (result.success) {
          setStoredMetadata(result.stored_metadata);
          if (result.embedded_metadata) {
            setEmbeddedMetadata(result.embedded_metadata.metadata);
            setEmbeddedFileType(result.embedded_metadata.file_type);
          }
          setPrimaryFile(result.primary_file);
        } else {
          setError('Failed to load metadata');
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    }
    fetchMetadata();
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
                  <span className="text-muted-foreground truncate" title={key}>
                    {formatFieldName(key)}
                  </span>
                  <span className="truncate" title={formatMetadataValue(key, value)}>
                    {urlValue ? (
                      <a
                        href={urlValue}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-primary hover:underline inline-flex items-center gap-1"
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
              className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1"
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
            <span className="text-muted-foreground truncate" title={key}>
              {formatFieldName(key)}
            </span>
            <span className="truncate" title={formatMetadataValue(key, value)}>
              {formatMetadataValue(key, value)}
            </span>
          </React.Fragment>
        ))}
      </div>
    );
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        className="bg-background border border-border rounded-lg shadow-lg w-full max-w-2xl max-h-[80vh] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border">
          <h2 className="text-lg font-semibold truncate">{modelName}</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-muted rounded"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="w-6 h-6 animate-spin text-muted-foreground" />
              <span className="ml-2 text-muted-foreground">Loading metadata...</span>
            </div>
          ) : error ? (
            <div className="text-center py-8 text-destructive">{error}</div>
          ) : (
            <div className="space-y-4">
              {/* Source toggle */}
              <div className="flex gap-2">
                <button
                  onClick={() => setActiveSource('embedded')}
                  className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm ${
                    activeSource === 'embedded'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-muted hover:bg-muted/80'
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
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-muted hover:bg-muted/80'
                  }`}
                  disabled={!storedMetadata}
                >
                  <Database className="w-4 h-4" />
                  Stored
                </button>
              </div>

              {/* Metadata display */}
              {activeSource === 'embedded' && embeddedMetadata ? (
                renderMetadataGrid(embeddedMetadata, embeddedFileType === 'gguf')
              ) : activeSource === 'stored' && storedMetadata ? (
                renderMetadataGrid(storedMetadata, false)
              ) : (
                <div className="text-center py-4 text-muted-foreground">
                  No {activeSource} metadata available
                </div>
              )}

              {/* Primary file path */}
              {primaryFile && (
                <div className="pt-2 border-t border-border">
                  <span className="text-xs text-muted-foreground">File: </span>
                  <span className="text-xs font-mono truncate">{primaryFile}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
