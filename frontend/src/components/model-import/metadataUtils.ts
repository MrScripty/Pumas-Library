import {
  Eye,
  Link,
  Shield,
  ShieldAlert,
  ShieldCheck,
  ShieldQuestion,
} from 'lucide-react';
import type { SecurityTier, HFMetadataLookupResult } from '../../types/api';

export function getFilename(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function getSecurityTier(filename: string): SecurityTier {
  const lower = filename.toLowerCase();
  if (lower.endsWith('.safetensors') || lower.endsWith('.gguf') || lower.endsWith('.onnx')) {
    return 'safe';
  }
  if (lower.endsWith('.ckpt') || lower.endsWith('.pt') || lower.endsWith('.bin') || lower.endsWith('.pth')) {
    return 'pickle';
  }
  return 'unknown';
}

export function getSecurityBadge(tier: SecurityTier): {
  className: string;
  text: string;
  Icon: typeof Shield;
} {
  switch (tier) {
    case 'safe':
      return {
        className: 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]',
        text: 'Safe Format',
        Icon: Shield,
      };
    case 'pickle':
      return {
        className: 'bg-[hsl(var(--launcher-accent-error)/0.2)] text-[hsl(var(--launcher-accent-error))]',
        text: 'Pickle Format',
        Icon: ShieldAlert,
      };
    default:
      return {
        className: 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]',
        text: 'Unknown Format',
        Icon: ShieldQuestion,
      };
  }
}

export function getTrustBadge(metadata?: HFMetadataLookupResult): {
  className: string;
  text: string;
  Icon: typeof ShieldCheck;
  tooltip: string;
} | null {
  if (!metadata || !metadata.match_method) return null;

  if (metadata.match_method === 'hash' && metadata.match_confidence === 1.0) {
    return {
      className: 'bg-[hsl(var(--launcher-accent-success)/0.2)] text-[hsl(var(--launcher-accent-success))]',
      text: 'Verified',
      Icon: ShieldCheck,
      tooltip: 'Hash matches HuggingFace - file is authentic',
    };
  }

  if (metadata.match_method === 'filename_exact') {
    return {
      className: 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--launcher-accent-primary))]',
      text: 'Matched',
      Icon: Link,
      tooltip: `Filename matched: ${metadata.repo_id}`,
    };
  }

  if (metadata.match_method === 'filename_fuzzy') {
    const confidence = metadata.match_confidence ?? 0;
    return {
      className: 'bg-[hsl(var(--launcher-accent-warning)/0.2)] text-[hsl(var(--launcher-accent-warning))]',
      text: 'Possible Match',
      Icon: Eye,
      tooltip: `Possible match: ${metadata.repo_id} (${Math.round(confidence * 100)}% confidence)`,
    };
  }

  return null;
}

const FIELD_PRIORITY: Record<string, number> = {
  official_name: 1,
  family: 2,
  model_type: 3,
  subtype: 4,
  variant: 5,
  precision: 6,
  base_model: 7,
  tags: 8,
  description: 9,
  match_confidence: 10,
  match_method: 11,
  matched_filename: 12,
};

export const EXCLUDED_FIELDS = new Set([
  'repo_id',
  'requires_confirmation',
  'hash_mismatch',
  'pending_full_verification',
  'fast_hash',
  'expected_sha256',
  'download_url',
]);

export function sortMetadataFields(keys: string[]): string[] {
  return [...keys].sort((a, b) => {
    const priorityA = FIELD_PRIORITY[a] ?? 999;
    const priorityB = FIELD_PRIORITY[b] ?? 999;
    if (priorityA !== priorityB) return priorityA - priorityB;
    return a.localeCompare(b);
  });
}

export function formatFieldName(key: string): string {
  return key
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function formatMetadataValue(key: string, value: unknown): string {
  if (value == null) return '';
  if (Array.isArray(value)) return value.join(', ');
  if (key === 'match_confidence' && typeof value === 'number') {
    return `${Math.round(value * 100)}%`;
  }
  if (typeof value === 'boolean') return value ? 'Yes' : 'No';
  return String(value);
}

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

const PRIORITY_ARCH_PATTERNS = [
  '.context_length',
  '.embedding_length',
];

export const LINKED_GGUF_FIELDS: Record<string, string> = {
  'general.basename': 'general.base_model.0.repo_url',
  'general.license': 'general.license.link',
  'general.quantized_by': 'general.repo_url',
};

const URL_TARGET_FIELDS = new Set(Object.values(LINKED_GGUF_FIELDS));

const HIDDEN_GGUF_FIELDS = new Set([
  'tokenizer.chat_template',
  'tokenizer.ggml.merges',
  'tokenizer.ggml.tokens',
  'tokenizer.ggml.token_type',
  'tokenizer.ggml.scores',
]);

export function isPriorityGgufField(key: string): boolean {
  const lowerKey = key.toLowerCase();
  if (PRIORITY_GGUF_FIELDS.some((field) => lowerKey === field)) return true;
  if (PRIORITY_ARCH_PATTERNS.some((pattern) => lowerKey.endsWith(pattern))) return true;
  return false;
}

export function isHiddenGgufField(key: string, value: unknown): boolean {
  const lowerKey = key.toLowerCase();
  if (HIDDEN_GGUF_FIELDS.has(lowerKey)) return true;
  if (URL_TARGET_FIELDS.has(lowerKey)) return true;
  const stringValue = String(value ?? '');
  if (stringValue.length > 500) return true;
  return false;
}

export function constructQuantUrl(embeddedMetadata: Record<string, unknown>): string | null {
  const quantizedBy = embeddedMetadata['general.quantized_by'];
  const name = embeddedMetadata['general.name'];

  if (!quantizedBy || !name) return null;

  return `https://huggingface.co/${String(quantizedBy)}/${String(name)}`;
}
