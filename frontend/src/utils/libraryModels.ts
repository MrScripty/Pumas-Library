import type { ModelCategory, ModelInfo } from '../types/apps';
import type { CatalogModel } from '@pumas/desktop-contract';
import { modelCategoryRank } from './modelCategories';

function getConvertibleFormat(format?: string): ModelInfo['primaryFormat'] {
  return format === 'gguf' || format === 'safetensors' ? format : undefined;
}

/** One projection owns both full catalog and catalog-search row semantics. */
export function projectCatalogModel(model: CatalogModel): ModelInfo {
  const partial = model.artifact.state === 'partial' ? model.artifact : undefined;
  return {
    provenance: 'catalog',
    id: model.id,
    name: model.displayName,
    category: model.modelType,
    inputModalities: model.inputModalities,
    outputModalities: model.outputModalities,
    adapter: model.adapter,
    path: model.id,
    modelDir: model.modelDir,
    format: model.format,
    quant: model.quantization,
    size: model.sizeBytes,
    date: model.displayDate,
    relatedAvailable: model.relatedAvailable,
    isPartialDownload: Boolean(partial),
    downloadProgress: partial?.downloadProgressFraction,
    recovery: partial?.recovery,
    hasDependencies: model.dependencyCount > 0,
    dependencyCount: model.dependencyCount,
    hasIntegrityIssue: model.integrity.state === 'duplicate',
    integrityIssueMessage: model.integrity.state === 'duplicate'
      ? `Duplicate artifact entries detected (${model.integrity.count} paths). Run library reconciliation.`
      : undefined,
    primaryFormat: getConvertibleFormat(model.format),
  };
}

export function groupCatalogModels(models: readonly CatalogModel[]): ModelCategory[] {
  const categoryMap = new Map<string, ModelInfo[]>();
  for (const model of models) {
    const row = projectCatalogModel(model);
    const group = categoryMap.get(row.category);
    if (group) group.push(row);
    else categoryMap.set(row.category, [row]);
  }
  return Array.from(categoryMap, ([category, models]) => ({ category, models }))
    .sort((a, b) => modelCategoryRank(a.category) - modelCategoryRank(b.category)
      || a.category.localeCompare(b.category));
}
