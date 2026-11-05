/**
 * The only way the renderer leaves the application: hand an http(s) URL to the
 * desktop bridge, which validates it again and opens the system browser. The
 * renderer never opens child windows or navigates itself.
 */

import { api, isAPIAvailable } from '../../api/adapter';
import { getLogger } from '../../utils/logger';

const logger = getLogger('openExternalUrl');

export function isExternalHttpUrl(url: unknown): url is string {
  if (typeof url !== 'string') return false;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

/** Open a URL in the system browser. Resolves false, with a logged reason, when it cannot. */
export async function openExternalUrl(url: string): Promise<boolean> {
  if (!isExternalHttpUrl(url)) {
    logger.warn('Refused external link: only http/https URLs open in the system browser');
    return false;
  }
  if (!isAPIAvailable()) {
    logger.warn('External link unavailable: the system browser opens only from the desktop shell', { url });
    return false;
  }
  try {
    const result = await api.open_url(url);
    if (!result.success) {
      logger.warn('Desktop bridge refused to open external link', { url, error: result.error });
      return false;
    }
    return true;
  } catch (error) {
    logger.error('Failed to open external link', {
      url,
      error: error instanceof Error ? error.message : String(error),
    });
    return false;
  }
}
