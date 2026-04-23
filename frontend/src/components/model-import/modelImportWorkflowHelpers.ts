import type {
  BundleComponentManifestEntry,
  BundleFormat,
  HFMetadataLookupResult,
  ImportPathClassification,
  ModelImportSpec,
  ShardedSetGroup,
} from '../../types/api';
import { getFilename, getSecurityTier } from './metadataUtils';
import type {
  DirectoryReviewFinding,
  ImportEntryKind,
  ImportEntryStatus,
  ShardedSetInfo,
} from './modelImportWorkflowTypes';

function pathStem(name: string): string {
  return name.replace(/\.[^.]+$/, '');
}

function createEntry(
  path: string,
  originPath: string,
  kind: ImportEntryKind,
  filename: string,
  suggestedFamily: string,
  suggestedOfficialName: string,
  modelType?: string,
  bundleFormat?: BundleFormat,
  pipelineClass?: string,
  componentManifest?: BundleComponentManifestEntry[],
  containerPath?: string
): ImportEntryStatus {
  const securityTier = kind === 'single_file' ? getSecurityTier(filename) : 'unknown';
  return {
    path,
    originPath,
    filename,
    kind,
    status: 'pending',
    securityTier,
    securityAcknowledged: securityTier !== 'pickle',
    metadataStatus:
      kind === 'single_file' || kind === 'external_diffusers_bundle' ? 'pending' : 'manual',
    suggestedFamily,
    suggestedOfficialName,
    modelType,
    bundleFormat,
    pipelineClass,
    componentManifest,
    containerPath,
  };
}

export function preferredBundleFamily(entry: ImportEntryStatus): string {
  const repoOwner = entry.hfMetadata?.repo_id?.split('/')[0]?.trim();
  if (repoOwner) {
    return repoOwner;
  }
  return entry.hfMetadata?.family || entry.suggestedFamily;
}

function createSingleResultEntry(result: ImportPathClassification): ImportEntryStatus | null {
  const suggestedFamily = result.suggested_family || 'imported';
  const suggestedOfficialName = result.suggested_official_name || pathStem(getFilename(result.path));

  if (result.kind === 'single_file') {
    return createEntry(
      result.path,
      result.path,
      'single_file',
      getFilename(result.path),
      suggestedFamily,
      suggestedOfficialName,
      result.model_type || undefined
    );
  }

  if (result.kind === 'single_model_directory') {
    return createEntry(
      result.path,
      result.path,
      'directory_model',
      getFilename(result.path),
      suggestedFamily,
      suggestedOfficialName,
      result.model_type || undefined
    );
  }

  if (result.kind !== 'single_bundle') {
    return null;
  }

  return createEntry(
    result.path,
    result.path,
    'external_diffusers_bundle',
    getFilename(result.path),
    suggestedFamily,
    suggestedOfficialName,
    result.model_type || undefined,
    result.bundle_format || undefined,
    result.pipeline_class || undefined,
    result.component_manifest || undefined
  );
}

function candidateEntryKind(kind: ImportPathClassification['candidates'][number]['kind']): ImportEntryKind {
  if (kind === 'external_diffusers_bundle') {
    return 'external_diffusers_bundle';
  }
  if (kind === 'directory_model') {
    return 'directory_model';
  }
  return 'single_file';
}

function createContainerEntries(result: ImportPathClassification): ImportEntryStatus[] {
  if (result.kind !== 'multi_model_container') {
    return [];
  }

  return result.candidates.map((candidate) => {
    const candidateFilename = candidate.display_name || getFilename(candidate.path);
    return createEntry(
      candidate.path,
      result.path,
      candidateEntryKind(candidate.kind),
      candidateFilename,
      'imported',
      pathStem(candidateFilename),
      candidate.model_type || undefined,
      candidate.bundle_format || undefined,
      candidate.pipeline_class || undefined,
      candidate.component_manifest || undefined,
      result.path
    );
  });
}

export function buildEntries(results: ImportPathClassification[]): ImportEntryStatus[] {
  const entries: ImportEntryStatus[] = [];
  const seenPaths = new Set<string>();

  const pushEntry = (entry: ImportEntryStatus) => {
    if (seenPaths.has(entry.path)) return;
    seenPaths.add(entry.path);
    entries.push(entry);
  };

  for (const result of results) {
    const singleEntry = createSingleResultEntry(result);
    if (singleEntry) {
      pushEntry(singleEntry);
    }

    for (const entry of createContainerEntries(result)) {
      pushEntry(entry);
    }
  }

  return entries;
}

export function buildReviewFindings(
  results: ImportPathClassification[]
): DirectoryReviewFinding[] {
  return results
    .filter(
      (result): result is ImportPathClassification & { kind: DirectoryReviewFinding['kind'] } => (
        result.kind === 'multi_model_container'
        || result.kind === 'ambiguous'
        || result.kind === 'unsupported'
      )
    )
    .map((result) => ({
      path: result.path,
      kind: result.kind,
      reasons: result.reasons,
      candidates: result.candidates,
    }));
}

export function buildShardedSetState(groups: Record<string, ShardedSetGroup>): {
  fileToSetMap: Record<string, string>;
  sets: ShardedSetInfo[];
} {
  const sets: ShardedSetInfo[] = [];
  const fileToSetMap: Record<string, string> = {};

  Object.entries(groups).forEach(([key, group]) => {
    if (group.files.length <= 1) {
      return;
    }

    sets.push({
      key,
      files: group.files,
      complete: group.validation.complete,
      missingShards: group.validation.missing_shards,
      expanded: false,
    });

    group.files.forEach((file) => {
      fileToSetMap[file] = key;
    });
  });

  return { fileToSetMap, sets };
}

export function extractEmbeddedRepoId(metadata: Record<string, unknown>): string | null {
  const repoUrl = metadata['general.repo_url'];
  if (typeof repoUrl === 'string') {
    const match = repoUrl.match(/huggingface\.co\/([^/]+\/[^/]+)/);
    if (match?.[1]) {
      return match[1];
    }
  }

  const quantizedBy = metadata['general.quantized_by'];
  const name = metadata['general.name'];
  if (quantizedBy && name) {
    return `${String(quantizedBy)}/${String(name)}`;
  }

  return null;
}

export function buildEmbeddedMetadataMatch(
  entry: ImportEntryStatus,
  repoId: string
): HFMetadataLookupResult {
  return {
    repo_id: repoId,
    official_name: entry.suggestedOfficialName,
    family: entry.suggestedFamily,
    match_method: 'filename_exact',
    match_confidence: 0.9,
    requires_confirmation: false,
  };
}

export function buildImportBatchSpecs(entries: ImportEntryStatus[]): ModelImportSpec[] {
  return entries.map((entry) => ({
    path: entry.path,
    family:
      entry.kind === 'external_diffusers_bundle'
        ? preferredBundleFamily(entry)
        : entry.hfMetadata?.family || entry.suggestedFamily,
    official_name: entry.hfMetadata?.official_name || entry.suggestedOfficialName,
    repo_id: entry.hfMetadata?.repo_id,
    model_type:
      entry.kind === 'external_diffusers_bundle'
        ? 'diffusion'
        : entry.hfMetadata?.model_type || entry.modelType,
    subtype: entry.hfMetadata?.subtype,
    tags: entry.hfMetadata?.tags,
    security_acknowledged: entry.securityAcknowledged,
  }));
}
