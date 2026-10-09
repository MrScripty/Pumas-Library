import type { RemoteModelInfo } from '../types/apps';

/** Discovery totals and quant tags alone do not select downloadable files. */
export function hasExactDownloadDetails(model: RemoteModelInfo): boolean {
  return Boolean(model.downloadOptions?.length) && (
    (typeof model.totalSizeBytes === 'number' && model.totalSizeBytes > 0)
    || (model.downloadOptions?.some(option =>
      (typeof option.sizeBytes === 'number' && option.sizeBytes > 0) || Boolean(option.fileGroup)
    ) ?? false)
  );
}
