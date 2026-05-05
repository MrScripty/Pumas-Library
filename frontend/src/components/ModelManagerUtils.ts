import type { DownloadStatus } from '../hooks/modelDownloadState';
import type { ModelCategory, ModelInfo, RemoteModelInfo } from '../types/apps';
import { getReleaseTimestamp } from '../utils/modelFormatters';

/** Detect HTTP 401 Unauthorized errors that indicate missing HF authentication. */
export function isAuthRequiredError(errorMessage: string): boolean {
  return /\b401\b/.test(errorMessage);
}

export function buildDownloadingModels(
  downloadStatusByRepo: Record<string, DownloadStatus>
): ModelInfo[] {
  return Object.entries(downloadStatusByRepo)
    .filter(([, status]) =>
      ['queued', 'downloading', 'cancelling', 'pausing', 'paused', 'error'].includes(status.status)
    )
    .map(([downloadKey, status]) => {
      const repoId = status.repoId || downloadKey;
      const name = status.modelName || repoId.split('/').pop() || repoId;
      return {
        id: `download:${downloadKey}`,
        name,
        category: status.modelType || 'llm',
        path: repoId,
        size: status.totalBytes,
        isDownloading: true,
        downloadProgress: status.progress,
        downloadStatus: status.status as ModelInfo['downloadStatus'],
        downloadKey,
        downloadRepoId: repoId,
        downloadSelectedArtifactId: status.selectedArtifactId,
        downloadArtifactId: status.artifactId,
        downloadTotalBytes: status.totalBytes,
      } as ModelInfo;
    });
}

function normalizeIdentity(value?: string | null): string | undefined {
  const normalized = value?.trim().toLowerCase();
  return normalized || undefined;
}

function normalizeArtifactToken(value?: string | null): string | undefined {
  const normalized = normalizeIdentity(value)?.replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '');
  return normalized || undefined;
}

function getDownloadArtifactIdentity(download: ModelInfo): string | undefined {
  return normalizeIdentity(download.downloadSelectedArtifactId ?? download.downloadArtifactId);
}

function getModelArtifactIdentity(model: ModelInfo): string | undefined {
  return normalizeIdentity(model.selectedArtifactId);
}

function isFileGroupOrRepoArtifactIdentity(artifactIdentity: string): boolean {
  const normalized = normalizeIdentity(artifactIdentity);
  const artifactToken = normalizeArtifactToken(artifactIdentity);
  return Boolean(
    normalized &&
      artifactToken &&
      (normalized.endsWith('__full_repo') ||
        normalized.includes('__files_') ||
        artifactToken.endsWith('_full_repo') ||
        artifactToken.includes('_files_'))
  );
}

function artifactIdentityContainsQuant(artifactIdentity: string, quant?: string | null): boolean {
  const quantToken = normalizeArtifactToken(quant);
  if (!quantToken) {
    return false;
  }

  const artifactToken = normalizeArtifactToken(artifactIdentity);
  return Boolean(
    artifactToken &&
      (artifactToken === quantToken ||
        artifactToken.endsWith(`_${quantToken}`) ||
        artifactToken.includes(`__${quantToken}`))
  );
}

function downloadMatchesLocalModel(model: ModelInfo, download: ModelInfo): boolean {
  const modelRepoId = normalizeIdentity(model.repoId);
  const downloadRepoId = normalizeIdentity(download.downloadRepoId);
  if (!modelRepoId || !downloadRepoId || modelRepoId !== downloadRepoId) {
    return false;
  }

  const downloadArtifactIdentity = getDownloadArtifactIdentity(download);
  if (!downloadArtifactIdentity) {
    return true;
  }

  const modelArtifactIdentity = getModelArtifactIdentity(model);
  if (modelArtifactIdentity) {
    return modelArtifactIdentity === downloadArtifactIdentity;
  }

  return (
    artifactIdentityContainsQuant(downloadArtifactIdentity, model.selectedArtifactQuant) ||
    artifactIdentityContainsQuant(downloadArtifactIdentity, model.quant) ||
    (Boolean(model.isPartialDownload) && isFileGroupOrRepoArtifactIdentity(downloadArtifactIdentity))
  );
}

function mergeDownloadState(model: ModelInfo, download: ModelInfo): ModelInfo {
  return {
    ...model,
    isDownloading: true,
    downloadProgress: download.downloadProgress,
    downloadStatus: download.downloadStatus,
    downloadKey: download.downloadKey,
    downloadRepoId: download.downloadRepoId,
    downloadSelectedArtifactId: download.downloadSelectedArtifactId,
    downloadArtifactId: download.downloadArtifactId,
    downloadTotalBytes: download.downloadTotalBytes,
  };
}

export function mergeLocalModelGroups(
  modelGroups: ModelCategory[],
  downloadingModels: ModelInfo[]
): ModelCategory[] {
  if (downloadingModels.length === 0) {
    return modelGroups;
  }

  const mergedDownloadKeys = new Set<string>();
  const groupMap = new Map<string, ModelInfo[]>();
  const repoIdsWithPartialRows = new Set(
    modelGroups.flatMap((group) =>
      group.models
        .filter((model) => model.isPartialDownload)
        .map((model) => normalizeIdentity(model.repoId))
        .filter((repoId): repoId is string => Boolean(repoId))
    )
  );

  modelGroups.forEach((group) => {
    const models = group.models.map((model) => {
      const download = downloadingModels.find((candidate) =>
        !mergedDownloadKeys.has(candidate.downloadKey ?? candidate.id) &&
        !(
          !model.isPartialDownload &&
          !getDownloadArtifactIdentity(candidate) &&
          repoIdsWithPartialRows.has(normalizeIdentity(model.repoId) ?? '')
        ) &&
        downloadMatchesLocalModel(model, candidate)
      );
      if (download) {
        mergedDownloadKeys.add(download.downloadKey ?? download.id);
        return mergeDownloadState(model, download);
      }
      return model;
    });
    groupMap.set(group.category, models);
  });

  const orphanDownloads = downloadingModels.filter(
    (download) => !mergedDownloadKeys.has(download.downloadKey ?? download.id)
  );
  orphanDownloads.forEach((model) => {
    const existing = groupMap.get(model.category);
    if (existing) {
      groupMap.set(model.category, [model, ...existing]);
    } else {
      groupMap.set(model.category, [model]);
    }
  });

  const orderedCategories = Array.from(
    new Set([
      ...modelGroups.map((group) => group.category),
      ...orphanDownloads.map((model) => model.category),
    ])
  );

  return orderedCategories.map((category) => ({
    category,
    models: groupMap.get(category) || [],
  }));
}

export function filterLocalModelGroups(
  localModelGroups: ModelCategory[],
  searchQuery: string,
  selectedCategory: string
): ModelCategory[] {
  let groups = localModelGroups;

  if (selectedCategory !== 'all') {
    groups = groups.filter((group) => group.category === selectedCategory);
  }

  if (!searchQuery.trim()) {
    return groups;
  }

  const query = searchQuery.toLowerCase();
  return groups
    .map((group) => ({
      ...group,
      models: group.models.filter(
        (model) =>
          model.name.toLowerCase().includes(query) || model.path?.toLowerCase().includes(query)
      ),
    }))
    .filter((group) => group.models.length > 0);
}

export function sortAndFilterRemoteResults(
  remoteResults: RemoteModelInfo[],
  selectedKind: string
): RemoteModelInfo[] {
  const filtered =
    selectedKind === 'all'
      ? remoteResults
      : remoteResults.filter((model) => model.kind === selectedKind);

  return [...filtered].sort(
    (a, b) => getReleaseTimestamp(b.releaseDate) - getReleaseTimestamp(a.releaseDate)
  );
}

export function resolveDownloadModelType(kind: string): string {
  const pipelineTagToModelType: Record<string, string> = {
    'text-generation': 'llm',
    'text2text-generation': 'llm',
    'question-answering': 'llm',
    'token-classification': 'llm',
    'text-classification': 'llm',
    'fill-mask': 'llm',
    'translation': 'llm',
    'summarization': 'llm',
    'conversational': 'llm',
    'text-ranking': 'reranker',
    'text-to-image': 'diffusion',
    'image-to-image': 'diffusion',
    'unconditional-image-generation': 'diffusion',
    'image-inpainting': 'diffusion',
    'text-to-video': 'diffusion',
    'text-to-3d': 'diffusion',
    'image-to-3d': 'diffusion',
    'text-to-audio': 'audio',
    'text-to-speech': 'audio',
    'automatic-speech-recognition': 'audio',
    'audio-classification': 'audio',
    'audio-to-audio': 'audio',
    'voice-activity-detection': 'audio',
    'image-classification': 'vision',
    'image-segmentation': 'vision',
    'object-detection': 'vision',
    'mask-generation': 'vision',
    'zero-shot-image-classification': 'vision',
    'depth-estimation': 'vision',
    'image-feature-extraction': 'vision',
    'zero-shot-object-detection': 'vision',
    'image-to-text': 'vlm',
    'image-text-to-text': 'vlm',
    'visual-question-answering': 'vlm',
    'document-question-answering': 'vlm',
    'video-classification': 'vision',
    'video-text-to-text': 'vlm',
    'feature-extraction': 'embedding',
    'sentence-similarity': 'embedding',
  };

  return pipelineTagToModelType[kind.toLowerCase()] ?? 'unknown';
}
