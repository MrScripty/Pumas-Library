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
        provenance: 'activity',
        id: `download:${downloadKey}`,
        name,
        category: status.modelType || 'llm',
        path: repoId,
        size: status.totalBytes,
        isDownloading: true,
        downloadProgress: status.progress,
        downloadStatus: status.status as ModelInfo['downloadStatus'],
        downloadKey,
        downloadLibraryModelId: status.libraryModelId,
        downloadRepoId: repoId,
        downloadSelectedArtifactId: status.selectedArtifactId,
        downloadArtifactId: status.artifactId,
        downloadTotalBytes: status.totalBytes,
      };
    });
}

export function mergeLocalModelGroups(
  modelGroups: ModelCategory[],
  downloadingModels: ModelInfo[]
): ModelCategory[] {
  if (downloadingModels.length === 0) {
    return modelGroups;
  }

  // Only an exact, unambiguous producer association can overlay a current row.
  // Labels and cached records cannot establish that relationship.
  const catalogCounts = new Map<string, number>();
  for (const group of modelGroups) {
    for (const model of group.models) {
      if (model.provenance === 'catalog') {
        catalogCounts.set(model.id, (catalogCounts.get(model.id) ?? 0) + 1);
      }
    }
  }
  const associated = new Map<string, ModelInfo[]>();
  for (const activity of downloadingModels) {
    const id = activity.downloadLibraryModelId;
    if (id) associated.set(id, [...(associated.get(id) ?? []), activity]);
  }
  const mergedActivities = new Set<ModelInfo>();
  const groupMap = new Map<string, ModelInfo[]>(
    modelGroups.map((group) => [group.category, group.models.map((model) => {
      const matches = associated.get(model.id);
      const activity = matches?.length === 1 ? matches[0] : undefined;
      if (model.provenance !== 'catalog' || catalogCounts.get(model.id) !== 1 || !activity) return model;
      mergedActivities.add(activity);
      return {
        ...model,
        isDownloading: activity.isDownloading,
        downloadProgress: activity.downloadProgress,
        downloadStatus: activity.downloadStatus,
        downloadKey: activity.downloadKey,
        downloadLibraryModelId: activity.downloadLibraryModelId,
        downloadRepoId: activity.downloadRepoId,
        downloadSelectedArtifactId: activity.downloadSelectedArtifactId,
        downloadArtifactId: activity.downloadArtifactId,
        downloadTotalBytes: activity.downloadTotalBytes,
      };
    })])
  );
  downloadingModels.forEach((model) => {
    if (mergedActivities.has(model)) return;
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
      ...downloadingModels.filter((model) => !mergedActivities.has(model)).map((model) => model.category),
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
    'text-classification': 'decision',
    'zero-shot-classification': 'decision',
    'fill-mask': 'llm',
    'translation': 'llm',
    'summarization': 'llm',
    'conversational': 'llm',
    'text-ranking': 'reranker',
    'text-to-image': 'diffusion',
    'image-to-image': 'diffusion',
    'unconditional-image-generation': 'diffusion',
    'image-inpainting': 'diffusion',
    'text-to-video': 'video',
    'image-to-video': 'video',
    'video-to-video': 'video',
    'image-text-to-video': 'video',
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
