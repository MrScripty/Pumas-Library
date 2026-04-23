import { api, isAPIAvailable } from '../api/adapter';
import { APIError, NetworkError } from '../errors';
import type { RemoteModelInfo } from '../types/apps';
import { getLogger } from '../utils/logger';
import {
  isAuthRequiredError,
  resolveDownloadModelType,
} from './ModelManagerUtils';

const logger = getLogger('ModelManager');

type SetDownloadErrors = (
  updater: (prev: Record<string, string>) => Record<string, string>
) => void;

type StartDownload = (
  repoId: string,
  downloadId: string,
  details?: { modelName?: string; modelType?: string }
) => void;

interface RemoteDownloadApi {
  start_model_download_from_hf: typeof api.start_model_download_from_hf;
}

interface RemoteDownloadLogger {
  error: (message: string, context?: Record<string, unknown>) => void;
  info: (message: string, context?: Record<string, unknown>) => void;
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

  const repoId = model.repoId;
  const developer = model.developer || repoId.split('/')[0] || 'huggingface';
  const officialName = model.name || repoId;
  const modelType = resolveDownloadModelType(model.kind || '');
  const pipelineTag = model.kind || '';
  const releaseDate = model.releaseDate || null;
  const downloadUrl = model.url || null;

  loggerInstance.info('Starting remote model download', {
    repoId,
    developer,
    officialName,
    modelType,
    quant,
    filenames: filenames?.length,
  });

  setDownloadErrors((prev) => {
    if (!prev[repoId]) return prev;
    const next = { ...prev };
    delete next[repoId];
    return next;
  });

  try {
    if (!isApiAvailable()) return;
    const result = await apiClient.start_model_download_from_hf(
      repoId,
      developer,
      officialName,
      modelType,
      pipelineTag,
      releaseDate,
      downloadUrl,
      quant || null,
      filenames || null
    );
    if (!result.success || !result.download_id) {
      const errorMsg = result.error || 'Download failed.';
      loggerInstance.error('Remote download failed', { error: errorMsg, repoId });
      setDownloadErrors((prev) => ({ ...prev, [repoId]: errorMsg }));
      return;
    }
    loggerInstance.info('Remote download started successfully', {
      repoId,
      downloadId: result.download_id,
    });
    startDownload(repoId, result.download_id, { modelName: officialName, modelType });
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Download failed.';
    if (error instanceof APIError) {
      loggerInstance.error('API error starting remote download', {
        error: error.message,
        endpoint: error.endpoint,
        repoId,
      });
    } else if (error instanceof NetworkError) {
      loggerInstance.error('Network error starting remote download', {
        error: error.message,
        url: error.url,
        status: error.status,
        repoId,
      });
    } else if (error instanceof Error) {
      loggerInstance.error('Failed to start remote download', { error: error.message, repoId });
    } else {
      loggerInstance.error('Unknown error starting remote download', { error, repoId });
    }
    setDownloadErrors((prev) => ({ ...prev, [repoId]: message }));
    if (isAuthRequiredError(message)) {
      openHfAuth();
    }
  }
}
