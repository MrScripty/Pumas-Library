import { ExternalLink, FileText } from 'lucide-react';
import type { VersionRelease } from '../hooks/useVersions';
import { formatVersionDate } from '../utils/installationFormatters';
import { IconButton } from './ui';

interface VersionListItemInfoProps {
  displayTag: string;
  errorMessage: string | null;
  failedLogPath: string | null;
  hasError: boolean;
  release: VersionRelease;
  onOpenLogPath: (path: string) => void;
  onOpenUrl: (url: string) => void;
}

export function VersionListItemInfo({
  displayTag,
  errorMessage,
  failedLogPath,
  hasError,
  release,
  onOpenLogPath,
  onOpenUrl,
}: VersionListItemInfoProps) {
  const releaseUrl = release.htmlUrl;

  return (
    <div className="flex-1 min-w-0">
      <div className="flex items-center gap-2">
        <div className="flex flex-col min-w-0">
          <div className="flex items-center gap-2 min-w-0">
            <h3 className="text-[hsl(var(--text-primary))] font-medium truncate">
              {displayTag}
            </h3>
            {releaseUrl && (
              <IconButton
                icon={<ExternalLink />}
                tooltip="Release notes"
                onClick={() => onOpenUrl(releaseUrl)}
                size="sm"
              />
            )}
            {failedLogPath && (
              <IconButton
                icon={<FileText className="text-[hsl(var(--accent-error))]" />}
                tooltip="View log"
                onClick={() => onOpenLogPath(failedLogPath)}
                size="sm"
              />
            )}
            {release.prerelease && (
              <span className="px-2 py-0.5 bg-[hsl(var(--accent-warning))]/20 text-[hsl(var(--accent-warning))] text-[11px] rounded-full">
                Pre
              </span>
            )}
          </div>
          <div className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))]">
            <span>{formatVersionDate(release.publishedAt)}</span>
          </div>
        </div>
      </div>

      {hasError && errorMessage && (
        <div className="mt-1 flex items-start gap-2 text-sm text-[hsl(var(--accent-error))] bg-[hsl(var(--accent-error))]/10 rounded p-2">
          <span>{errorMessage}</span>
        </div>
      )}
    </div>
  );
}
