import type { ModelCategory, ModelInfo } from '../types/apps';
import type { ModelRecord, ModelRecordMetadata } from '../types/api';

function asString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function asBoolean(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function getIntegrityIssueMessage(metadata: ModelRecordMetadata): string | undefined {
  if (!metadata.integrity_issue_duplicate_repo_id) {
    return undefined;
  }

  const count = metadata.integrity_issue_duplicate_repo_id_count ?? 2;
  return `Duplicate repo entries detected (${count} paths). Run library reconciliation.`;
}

function getConvertibleFormat(format?: string): ModelInfo['primaryFormat'] {
  if (format === 'gguf' || format === 'safetensors') {
    return format;
  }
  return undefined;
}

export function mapModelRecordToInfo(model: ModelRecord): ModelInfo {
  const metadata = model.metadata ?? {};
  const fileName = model.id.split('/').pop() || model.id;
  const displayName = model.officialName ?? model.cleanedName ?? fileName;
  const dependencyBindings = asArray(metadata.dependency_bindings);
  const format = asString(metadata.primary_format);
  const conversionSource = metadata.conversion_source as Record<string, unknown> | undefined;

  return {
    id: model.id,
    name: displayName,
    category: model.modelType || metadata.model_type || 'uncategorized',
    path: model.id,
    modelDir: model.path,
    format,
    quant: asString(metadata.quantization),
    size: asNumber(metadata.size_bytes),
    date: asString(metadata.added_date),
    relatedAvailable: asBoolean(metadata.related_available) ?? false,
    isPartialDownload: asBoolean(metadata.download_incomplete) ?? false,
    wasDequantized: asBoolean(conversionSource?.['was_dequantized']) ?? false,
    convertedFrom: asString(conversionSource?.['source_format']),
    repoId: asString(metadata.repo_id),
    hasDependencies: dependencyBindings.length > 0,
    dependencyCount: dependencyBindings.length || undefined,
    hasIntegrityIssue: asBoolean(metadata.integrity_issue_duplicate_repo_id) ?? false,
    integrityIssueMessage: getIntegrityIssueMessage(metadata),
    primaryFormat: getConvertibleFormat(format),
  };
}

export function groupModelRecords(models: ModelRecord[]): ModelCategory[] {
  const categoryMap = new Map<string, ModelInfo[]>();

  for (const model of models) {
    const modelInfo = mapModelRecordToInfo(model);
    const groupedModels = categoryMap.get(modelInfo.category);
    if (groupedModels) {
      groupedModels.push(modelInfo);
      continue;
    }

    categoryMap.set(modelInfo.category, [modelInfo]);
  }

  return Array.from(categoryMap.entries()).map(([category, groupedModels]) => ({
    category,
    models: groupedModels,
  }));
}
