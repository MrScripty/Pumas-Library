import { api, isAPIAvailable } from '../api/adapter';
import { APIError, NetworkError } from '../errors';
import type { RemoteModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import { getDownloadArtifactKey, type DownloadArtifactIdentity } from '../hooks/modelDownloadState';
import {
  isAuthRequiredError,
  resolveDownloadModelType,
} from './ModelManagerUtils';

const logger = getLogger('ModelManager');

type SetDownloadErrors = (
  updater: (prev: Record<string, string>) => Record<string, string>
) => void;

type StartDownload = (
  downloadKey: string,
  downloadId: string,
  details?: { modelName?: string; modelType?: string } & DownloadArtifactIdentity
) => void;

interface RemoteDownloadApi {
  start_model_download_from_hf: typeof api.start_model_download_from_hf;
}

interface RemoteDownloadLogger {
  error: (message: string, context?: Record<string, unknown>) => void;
  info: (message: string, context?: Record<string, unknown>) => void;
}

interface RemoteDownloadRequest {
  developer: string;
  downloadUrl: string | null;
  filenames: string[] | null;
  officialName: string;
  modelType: string;
  pipelineTag: string;
  quant: string | null;
  releaseDate: string | null;
  repoId: string;
}

export interface StartRemoteModelDownloadOptions {
  filenames?: string[] | null | undefined;
  model: RemoteModelInfo;
  quant?: string | null | undefined;
  openHfAuth: () => void;
  setDownloadErrors: SetDownloadErrors;
  startDownload: StartDownload;
  apiClient?: RemoteDownloadApi | undefined;
  isApiAvailable?: (() => boolean) | undefined;
  loggerInstance?: RemoteDownloadLogger | undefined;
}

function createRemoteDownloadRequest(
  model: RemoteModelInfo,
  quant?: string | null,
  filenames?: string[] | null
): RemoteDownloadRequest {
  const repoId = model.repoId;
  return {
    repoId,
    developer: model.developer || repoId.split('/')[0] || 'huggingface',
    officialName: model.name || repoId,
    modelType: resolveDownloadModelType(model.kind || ''),
    pipelineTag: model.kind || '',
    releaseDate: model.releaseDate || null,
    downloadUrl: model.url || null,
    quant: quant || null,
    filenames: filenames || null,
  };
}

function clearDownloadError(downloadKey: string, setDownloadErrors: SetDownloadErrors): void {
  setDownloadErrors((prev) => {
    if (!prev[downloadKey]) return prev;
    const next = { ...prev };
    delete next[downloadKey];
    return next;
  });
}

function recordRepoDownloadError(
  repoId: string,
  message: string,
  setDownloadErrors: SetDownloadErrors
): void {
  setDownloadErrors((prev) => ({ ...prev, [repoId]: message }));
}

function reportRemoteDownloadError(
  error: unknown,
  repoId: string,
  loggerInstance: RemoteDownloadLogger
): string {
  if (error instanceof APIError) {
    loggerInstance.error('API error starting remote download', {
      error: error.message,
      endpoint: error.endpoint,
      repoId,
    });
    return error.message;
  }

  if (error instanceof NetworkError) {
    loggerInstance.error('Network error starting remote download', {
      error: error.message,
      url: error.url,
      status: error.status,
      repoId,
    });
    return error.message;
  }

  if (error instanceof Error) {
    loggerInstance.error('Failed to start remote download', { error: error.message, repoId });
    return error.message;
  }

  loggerInstance.error('Unknown error starting remote download', { error, repoId });
  return 'Download failed.';
}

export async function startRemoteModelDownload({
  filenames,
  model,
  quant,
  openHfAuth,
  setDownloadErrors,
  startDownload,
  apiClient = api,
  isApiAvailable = isAPIAvailable,
  loggerInstance = logger,
}: StartRemoteModelDownloadOptions): Promise<void> {
  if (!isApiAvailable()) {
    loggerInstance.error('Download API not available');
    return;
  }

  const request = createRemoteDownloadRequest(model, quant, filenames);

  loggerInstance.info('Starting remote model download', {
    repoId: request.repoId,
    developer: request.developer,
    officialName: request.officialName,
    modelType: request.modelType,
    quant: request.quant,
    filenames: request.filenames?.length,
  });

  clearDownloadError(request.repoId, setDownloadErrors);

  try {
    if (!isApiAvailable()) return;
    const result = await apiClient.start_model_download_from_hf(
      request.repoId,
      request.developer,
      request.officialName,
      request.modelType,
      request.pipelineTag,
      request.releaseDate,
      request.downloadUrl,
      request.quant,
      request.filenames
    );
    if (!result.success || !result.download_id) {
      const errorMsg = result.error || 'Download failed.';
      loggerInstance.error('Remote download failed', { error: errorMsg, repoId: request.repoId });
      recordRepoDownloadError(request.repoId, errorMsg, setDownloadErrors);
      return;
    }
    loggerInstance.info('Remote download started successfully', {
      repoId: request.repoId,
      downloadId: result.download_id,
      selectedArtifactId: result.selectedArtifactId,
      artifactId: result.artifactId,
    });
    const downloadKey = getDownloadArtifactKey({
      repoId: request.repoId,
      selectedArtifactId: result.selectedArtifactId,
      artifactId: result.artifactId,
    }) ?? request.repoId;
    clearDownloadError(downloadKey, setDownloadErrors);
    startDownload(downloadKey, result.download_id, {
      repoId: request.repoId,
      selectedArtifactId: result.selectedArtifactId,
      artifactId: result.artifactId,
      modelName: request.officialName,
      modelType: request.modelType,
    });
  } catch (error) {
    const message = reportRemoteDownloadError(error, request.repoId, loggerInstance);
    recordRepoDownloadError(request.repoId, message, setDownloadErrors);
    if (isAuthRequiredError(message)) {
      openHfAuth();
    }
  }
}
