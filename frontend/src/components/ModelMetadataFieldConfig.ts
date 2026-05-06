import type { InferenceParamSchema } from '../types/api';

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
export const LINKED_GGUF_FIELDS: Record<string, string> = {
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

export type MetadataSource = 'stored' | 'embedded' | 'inference' | 'execution' | 'runtime' | 'notes';

export const PARAM_TYPE_OPTIONS: {
  value: InferenceParamSchema['param_type'];
  label: string;
}[] = [
  { value: 'Integer', label: 'Integer' },
  { value: 'Number', label: 'Number' },
  { value: 'String', label: 'String' },
  { value: 'Boolean', label: 'Boolean' },
];

export interface SelectAllowedOption {
  label: string;
  value: string;
}

/** Format field name for display */
export function formatFieldName(key: string): string {
  return key
    .replace(/^general\./, '')
    .replace(/\./g, ' ')
    .replace(/_/g, ' ')
    .split(' ')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

/** Format metadata value for display */
export function formatMetadataValue(key: string, value: unknown): string {
  if (value == null) return '';
  if (Array.isArray(value)) return value.join(', ');
  if (key === 'match_confidence' && typeof value === 'number') {
    return `${Math.round(value * 100)}%`;
  }
  if (typeof value === 'boolean') return value ? 'Yes' : 'No';
  return String(value);
}

/** Check whether value is a plain object-like record */
export function isRecordValue(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function normalizeAllowedOption(raw: unknown): SelectAllowedOption | null {
  if (typeof raw === 'string') {
    return { label: raw, value: raw };
  }
  if (!isRecordValue(raw)) {
    return null;
  }

  const value = typeof raw['value'] === 'string' ? raw['value'] : null;
  if (!value) {
    return null;
  }

  const label = typeof raw['label'] === 'string' && raw['label'].trim() !== ''
    ? raw['label']
    : value;
  return { label, value };
}

export function normalizeStringDefault(raw: unknown): string {
  if (typeof raw === 'string') {
    return raw;
  }
  if (isRecordValue(raw) && typeof raw['value'] === 'string') {
    return raw['value'];
  }
  return raw == null ? '' : String(raw);
}

export function getStoredNotes(metadata: Record<string, unknown> | null): string {
  const notes = metadata?.['notes'];
  return typeof notes === 'string' ? notes : '';
}

/** Check if a GGUF field is a priority field */
export function isPriorityGgufField(key: string): boolean {
  const lowerKey = key.toLowerCase();
  if (PRIORITY_GGUF_FIELDS.some((field) => lowerKey === field)) return true;
  if (PRIORITY_ARCH_PATTERNS.some((pattern) => lowerKey.endsWith(pattern))) return true;
  return false;
}

/** Check if a GGUF field should be hidden by default */
export function isHiddenGgufField(key: string, value: unknown): boolean {
  const lowerKey = key.toLowerCase();
  if (HIDDEN_GGUF_FIELDS.has(lowerKey)) return true;
  if (URL_TARGET_FIELDS.has(lowerKey)) return true;
  const stringValue = String(value ?? '');
  if (stringValue.length > 500) return true;
  return false;
}
