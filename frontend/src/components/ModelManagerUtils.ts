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
    .map(([repoId, status]) => {
      const name = status.modelName || repoId.split('/').pop() || repoId;
      return {
        id: `download:${repoId}`,
        name,
        category: status.modelType || 'llm',
        path: repoId,
        size: status.totalBytes,
        isDownloading: true,
        downloadProgress: status.progress,
        downloadStatus: status.status as ModelInfo['downloadStatus'],
        downloadRepoId: repoId,
        downloadTotalBytes: status.totalBytes,
      } as ModelInfo;
    });
}

export function mergeLocalModelGroups(
  modelGroups: ModelCategory[],
  downloadingModels: ModelInfo[]
): ModelCategory[] {
  if (downloadingModels.length === 0) {
    return modelGroups;
  }

  const downloadByRepoId = new Map<string, ModelInfo>();
  for (const download of downloadingModels) {
    if (!download.downloadRepoId) {
      continue;
    }

    const key = download.downloadRepoId.toLowerCase();
    if (!downloadByRepoId.has(key)) {
      downloadByRepoId.set(key, download);
    }
  }

  const mergedRepoKeys = new Set<string>();
  const groupMap = new Map<string, ModelInfo[]>();

  modelGroups.forEach((group) => {
    const models = group.models.map((model) => {
      const key = model.repoId?.toLowerCase();
      if (key && downloadByRepoId.has(key)) {
        const download = downloadByRepoId.get(key)!;
        mergedRepoKeys.add(key);
        return {
          ...model,
          isDownloading: true,
          downloadProgress: download.downloadProgress,
          downloadStatus: download.downloadStatus,
          downloadRepoId: download.downloadRepoId,
          downloadTotalBytes: download.downloadTotalBytes,
        };
      }
      return model;
    });
    groupMap.set(group.category, models);
  });

  const orphanDownloads = downloadingModels.filter(
    (download) => !download.downloadRepoId || !mergedRepoKeys.has(download.downloadRepoId.toLowerCase())
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
