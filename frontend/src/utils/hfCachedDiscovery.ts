import { APIError } from '../errors';
import type { RemoteModelInfo, RemoteSearchSource } from '../types/apps';

export function effectiveSearchSource(query: string, source: RemoteSearchSource): RemoteSearchSource {
  return source === 'cached' || query.trim().startsWith('cache:') ? 'cached' : 'huggingface';
}

export function presentSearchResults(models: RemoteModelInfo[], source: RemoteSearchSource): RemoteModelInfo[] {
  return models.map((model) => {
    // Public cardData and any preexisting consumer annotation cannot choose the
    // display source for an ordinary search response.
    if (source !== 'cached') {
      const ordinary = { ...model };
      delete ordinary.cachedDiscovery;
      return ordinary;
    }
    const value = model.modelCard?.['pumas_discovery'];
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
      throw new APIError('Cached search returned incomplete provenance.', 'search_hf_models');
    }
    const record = value as Record<string, unknown>;
    const validDate = (value: unknown): value is string => typeof value === 'string' && Number.isFinite(Date.parse(value));
    if (record['source'] !== 'anonymous-hf-detail-cache' || record['discovery_only'] !== true
        || record['visibility_observed'] !== 'public-ungated'
        || typeof record['source_url'] !== 'string' || record['source_url'].length > 2048
        || !validDate(record['observed_at'])
        || !['fresh', 'stale'].includes(record['freshness'] as string)
        || (record['fresh_until'] !== null && !validDate(record['fresh_until']))
        || (record['revision_observed'] !== null && (typeof record['revision_observed'] !== 'string'
          || !/^(?:[a-fA-F0-9]{40}|[a-fA-F0-9]{64})$/.test(record['revision_observed'])))) {
      throw new APIError('Cached search returned incomplete provenance.', 'search_hf_models');
    }
    let url: URL;
    try { url = new URL(record['source_url']); } catch { throw new APIError('Cached search returned an invalid source.', 'search_hf_models'); }
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.search || url.hash
        || !url.pathname.endsWith(`/models/${model.repoId}`)) {
      throw new APIError('Cached search returned an invalid source.', 'search_hf_models');
    }
    return { ...model, compatibleEngines: [], downloadOptions: [], cachedDiscovery: {
      sourceUrl: record['source_url'],
      observedAt: record['observed_at'],
      freshness: record['freshness'] as 'fresh' | 'stale',
      freshUntil: record['fresh_until'],
      revisionObserved: record['revision_observed'],
    } };
  });
}
