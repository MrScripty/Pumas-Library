import { ExternalLink } from 'lucide-react';
import type { RelatedModelsStatus, RemoteModelInfo } from '../types/apps';
import { ModelKindIcon } from './ModelKindIcon';
import { IconButton } from './ui';

interface RelatedModelsPanelProps {
  error?: string;
  onOpenRelatedUrl: (url: string) => void;
  relatedModels: RemoteModelInfo[];
  relatedStatus: RelatedModelsStatus;
}

export function RelatedModelsPanel({
  error,
  onOpenRelatedUrl,
  relatedModels,
  relatedStatus,
}: RelatedModelsPanelProps) {
  const isLoadingRelated = relatedStatus === 'loading' || relatedStatus === 'idle';

  return (
    <div className="border-t border-[hsl(var(--launcher-border))] px-3 py-2 space-y-2">
      <div className="flex items-center justify-between text-[11px] uppercase tracking-wider text-[hsl(var(--text-muted))]">
        <span>Related models</span>
        {relatedModels.length > 0 && <span>{relatedModels.length}</span>}
      </div>
      {isLoadingRelated && (
        <div className="text-xs text-[hsl(var(--text-muted))]">
          Looking up related models...
        </div>
      )}
      {relatedStatus === 'error' && (
        <div className="text-xs text-[hsl(var(--launcher-accent-error))]">
          {error || 'Related models unavailable.'}
        </div>
      )}
      {!isLoadingRelated && relatedStatus !== 'error' && relatedModels.length === 0 && (
        <div className="text-xs text-[hsl(var(--text-muted))]">
          No related models found.
        </div>
      )}
      {relatedModels.length > 0 && (
        <div className="space-y-1.5">
          {relatedModels.map((related) => (
            <div
              key={related.repoId}
              className="flex items-center justify-between rounded bg-[hsl(var(--launcher-bg-tertiary)/0.2)] px-2 py-1.5"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-semibold text-[hsl(var(--text-primary))] truncate">
                    {related.name}
                  </span>
                  <span
                    className="inline-flex items-center gap-1 text-[hsl(var(--text-muted))]"
                    title={related.kind}
                    aria-label={related.kind}
                  >
                    <ModelKindIcon kind={related.kind} />
                  </span>
                </div>
                <span className="text-[11px] text-[hsl(var(--text-muted))] truncate">
                  {related.developer}
                </span>
              </div>
              <IconButton
                icon={<ExternalLink />}
                tooltip="Open"
                onClick={() => onOpenRelatedUrl(related.url)}
                size="sm"
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
