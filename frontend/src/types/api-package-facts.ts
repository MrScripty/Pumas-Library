import type { AssetValidationError, AssetValidationState, StorageKind } from './api-import';

export type PackageArtifactKind =
  | 'gguf'
  | 'hf_compatible_directory'
  | 'safetensors'
  | 'diffusers_bundle'
  | 'onnx'
  | 'adapter'
  | 'shard'
  | 'unknown';

export type PackageFactStatus = 'present' | 'missing' | 'invalid' | 'unsupported' | 'uninspected';

export type ProcessorComponentKind =
  | 'config'
  | 'tokenizer'
  | 'tokenizer_config'
  | 'processor'
  | 'preprocessor'
  | 'image_processor'
  | 'video_processor'
  | 'audio_feature_extractor'
  | 'feature_extractor'
  | 'chat_template'
  | 'generation_config'
  | 'model_index'
  | 'weight_index'
  | 'weights'
  | 'adapter'
  | 'quantization'
  | 'other';

export type BackendHintLabel =
  | 'transformers'
  | 'llama.cpp'
  | 'vllm'
  | 'mlx'
  | 'candle'
  | 'diffusers'
  | 'onnx-runtime';

export interface ModelPackageDiagnostic {
  code: string;
  message: string;
  path?: string | null;
}

export interface ModelRefMigrationDiagnostic {
  code: string;
  message: string;
  input?: string | null;
}

export interface PumasModelRef {
  model_ref_contract_version?: number;
  model_id: string;
  revision?: string | null;
  selected_artifact_id?: string | null;
  selected_artifact_path?: string | null;
  migration_diagnostics?: ModelRefMigrationDiagnostic[];
}

export interface ResolvedArtifactFacts {
  artifact_kind: PackageArtifactKind;
  entry_path: string;
  storage_kind: StorageKind;
  validation_state: AssetValidationState;
  validation_errors?: AssetValidationError[];
  companion_artifacts?: string[];
  sibling_files?: string[];
  selected_files?: string[];
}

export interface ProcessorComponentFacts {
  kind: ProcessorComponentKind;
  status: PackageFactStatus;
  relative_path?: string | null;
  class_name?: string | null;
  message?: string | null;
}

export interface TransformersPackageEvidence {
  config_status: PackageFactStatus;
  config_model_type?: string | null;
  architectures?: string[];
  dtype?: string | null;
  torch_dtype?: string | null;
  auto_map?: string[];
  processor_class?: string | null;
  generation_config_status: PackageFactStatus;
  source_repo_id?: string | null;
  source_revision?: string | null;
  selected_files?: string[];
}

export interface TaskEvidence {
  pipeline_tag?: string | null;
  task_type_primary?: string | null;
  input_modalities?: string[];
  output_modalities?: string[];
}

export interface GenerationDefaultFacts {
  status: PackageFactStatus;
  source_path?: string | null;
  defaults?: unknown;
  diagnostics?: ModelPackageDiagnostic[];
}

export interface CustomCodeFacts {
  requires_custom_code: boolean;
  custom_code_sources?: string[];
  auto_map_sources?: string[];
  class_references?: PackageClassReference[];
  dependency_manifests?: string[];
}

export interface PackageClassReference {
  kind: ProcessorComponentKind;
  class_name: string;
  source_path?: string | null;
}

export interface BackendHintFacts {
  accepted?: BackendHintLabel[];
  raw?: string[];
  unsupported?: string[];
}

export interface ResolvedModelPackageFacts {
  package_facts_contract_version: number;
  model_ref: PumasModelRef;
  artifact: ResolvedArtifactFacts;
  components: ProcessorComponentFacts[];
  transformers?: TransformersPackageEvidence | null;
  task: TaskEvidence;
  generation_defaults: GenerationDefaultFacts;
  custom_code: CustomCodeFacts;
  backend_hints: BackendHintFacts;
  diagnostics?: ModelPackageDiagnostic[];
}

export type ModelFactFamily =
  | 'model_record'
  | 'metadata'
  | 'package_facts'
  | 'dependency_bindings'
  | 'validation'
  | 'search_index';

export type ModelLibraryChangeKind =
  | 'model_added'
  | 'model_removed'
  | 'metadata_modified'
  | 'package_facts_modified'
  | 'stale_facts_invalidated'
  | 'dependency_binding_modified';

export type ModelLibraryRefreshScope = 'summary' | 'detail' | 'summary_and_detail';

export interface ModelLibraryUpdateEvent {
  cursor: string;
  model_id: string;
  change_kind: ModelLibraryChangeKind;
  fact_family: ModelFactFamily;
  refresh_scope: ModelLibraryRefreshScope;
  selected_artifact_id?: string | null;
  producer_revision?: string | null;
}

export interface ModelLibraryUpdateFeed {
  cursor: string;
  events?: ModelLibraryUpdateEvent[];
  stale_cursor: boolean;
  snapshot_required: boolean;
}

export interface ModelLibraryUpdateNotification {
  cursor: string;
  events?: ModelLibraryUpdateEvent[];
  stale_cursor: boolean;
  snapshot_required: boolean;
}

const MODEL_FACT_FAMILIES: ReadonlySet<ModelFactFamily> = new Set([
  'model_record',
  'metadata',
  'package_facts',
  'dependency_bindings',
  'validation',
  'search_index',
]);

const MODEL_LIBRARY_CHANGE_KINDS: ReadonlySet<ModelLibraryChangeKind> = new Set([
  'model_added',
  'model_removed',
  'metadata_modified',
  'package_facts_modified',
  'stale_facts_invalidated',
  'dependency_binding_modified',
]);

const MODEL_LIBRARY_REFRESH_SCOPES: ReadonlySet<ModelLibraryRefreshScope> = new Set([
  'summary',
  'detail',
  'summary_and_detail',
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null | undefined {
  return value === undefined || value === null || typeof value === 'string';
}

export function isModelLibraryUpdateEvent(value: unknown): value is ModelLibraryUpdateEvent {
  if (!isRecord(value)) return false;
  return (
    typeof value['cursor'] === 'string' &&
    typeof value['model_id'] === 'string' &&
    MODEL_LIBRARY_CHANGE_KINDS.has(value['change_kind'] as ModelLibraryChangeKind) &&
    MODEL_FACT_FAMILIES.has(value['fact_family'] as ModelFactFamily) &&
    MODEL_LIBRARY_REFRESH_SCOPES.has(value['refresh_scope'] as ModelLibraryRefreshScope) &&
    isNullableString(value['selected_artifact_id']) &&
    isNullableString(value['producer_revision'])
  );
}

export function isModelLibraryUpdateNotification(
  value: unknown
): value is ModelLibraryUpdateNotification {
  if (!isRecord(value)) return false;
  if (
    typeof value['cursor'] !== 'string' ||
    typeof value['stale_cursor'] !== 'boolean' ||
    typeof value['snapshot_required'] !== 'boolean'
  ) {
    return false;
  }
  if (value['events'] === undefined) {
    return true;
  }
  return Array.isArray(value['events']) && value['events'].every(isModelLibraryUpdateEvent);
}

export type ModelPackageFactsSummaryStatus =
  | 'cached'
  | 'missing'
  | 'invalid'
  | 'fresh'
  | 'detail_derived'
  | 'regenerated';

export interface ResolvedModelPackageFactsSummary {
  package_facts_contract_version: number;
  model_ref: PumasModelRef;
  artifact_kind: PackageArtifactKind;
  entry_path: string;
  storage_kind: StorageKind;
  validation_state: AssetValidationState;
  task: TaskEvidence;
  backend_hints: BackendHintFacts;
  requires_custom_code: boolean;
  config_status: PackageFactStatus;
  tokenizer_status: PackageFactStatus;
  processor_status: PackageFactStatus;
  generation_config_status: PackageFactStatus;
  generation_defaults_status: PackageFactStatus;
  diagnostic_codes?: string[];
}

export interface ModelPackageFactsSummaryResult {
  model_id: string;
  status: ModelPackageFactsSummaryStatus;
  summary?: ResolvedModelPackageFactsSummary | null;
}

export interface ModelPackageFactsSummarySnapshotItem {
  model_id: string;
  status: ModelPackageFactsSummaryStatus;
  summary?: ResolvedModelPackageFactsSummary | null;
}

export interface ModelPackageFactsSummarySnapshot {
  cursor: string;
  items: ModelPackageFactsSummarySnapshotItem[];
}
