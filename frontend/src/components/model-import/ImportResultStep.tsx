import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Loader2,
  ShieldCheck,
} from 'lucide-react';
import { getTrustBadge } from './metadataUtils';
import type { ImportEntryStatus } from './useModelImportWorkflow';

interface ImportResultStepProps {
  entries: ImportEntryStatus[];
  failedCount: number;
  importedCount: number;
  mode: 'importing' | 'complete';
  verifiedCount: number;
}

export function ImportResultStep({
  entries,
  failedCount,
  importedCount,
  mode,
  verifiedCount,
}: ImportResultStepProps) {
  if (mode === 'importing') {
    return (
      <div className="space-y-4">
        <div className="flex items-center justify-center py-8">
          <Loader2 className="w-12 h-12 text-[hsl(var(--launcher-accent-primary))] animate-spin" />
        </div>
        <div className="space-y-2">
          {entries.map((entry) => (
            <div
              key={entry.path}
              className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
            >
              {entry.status === 'importing' && (
                <Loader2 className="w-4 h-4 text-[hsl(var(--launcher-accent-primary))] animate-spin flex-shrink-0" />
              )}
              {entry.status === 'success' && (
                <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
              )}
              {entry.status === 'error' && (
                <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
              )}
              {entry.status === 'pending' && (
                <div className="w-4 h-4 rounded-full border-2 border-[hsl(var(--launcher-border))] flex-shrink-0" />
              )}
              <span className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate flex-1">
                {entry.filename}
              </span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-center py-6">
        {failedCount === 0 ? (
          <div className="flex flex-col items-center">
            <CheckCircle2 className="w-16 h-16 text-[hsl(var(--launcher-accent-success))] mb-3" />
            <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
              {importedCount} item{importedCount !== 1 ? 's' : ''} imported successfully
            </p>
            {verifiedCount > 0 && (
              <p className="text-sm text-[hsl(var(--launcher-text-muted))] flex items-center gap-1 mt-1">
                <ShieldCheck className="w-4 h-4 text-[hsl(var(--launcher-accent-success))]" />
                {verifiedCount} verified from HuggingFace
              </p>
            )}
          </div>
        ) : importedCount === 0 ? (
          <div className="flex flex-col items-center">
            <AlertCircle className="w-16 h-16 text-[hsl(var(--launcher-accent-error))] mb-3" />
            <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
              Import failed
            </p>
            <p className="text-sm text-[hsl(var(--launcher-text-muted))]">
              {failedCount} item{failedCount !== 1 ? 's' : ''} could not be imported
            </p>
          </div>
        ) : (
          <div className="flex flex-col items-center">
            <AlertTriangle className="w-16 h-16 text-[hsl(var(--launcher-accent-warning))] mb-3" />
            <p className="text-lg font-medium text-[hsl(var(--launcher-text-primary))]">
              Partial import
            </p>
            <p className="text-sm text-[hsl(var(--launcher-text-muted))]">
              {importedCount} imported, {failedCount} failed
            </p>
          </div>
        )}
      </div>

      <div className="space-y-2">
        {entries.map((entry) => {
          const trustBadge = getTrustBadge(entry.hfMetadata);
          return (
            <div
              key={entry.path}
              className="flex items-center gap-3 p-3 rounded-lg bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
            >
              {entry.status === 'success' && (
                <CheckCircle2 className="w-4 h-4 text-[hsl(var(--launcher-accent-success))] flex-shrink-0" />
              )}
              {entry.status === 'error' && (
                <AlertCircle className="w-4 h-4 text-[hsl(var(--launcher-accent-error))] flex-shrink-0" />
              )}
              <div className="flex-1 min-w-0">
                <p className="text-sm text-[hsl(var(--launcher-text-secondary))] truncate">
                  {entry.filename}
                </p>
                {entry.error && (
                  <p className="text-xs text-[hsl(var(--launcher-accent-error))] truncate">
                    {entry.error}
                  </p>
                )}
              </div>
              {trustBadge && entry.status === 'success' && (
                <span
                  className={`px-2 py-0.5 rounded text-xs font-medium flex items-center gap-1 ${trustBadge.className}`}
                  title={trustBadge.tooltip}
                >
                  <trustBadge.Icon className="w-3 h-3" />
                  {trustBadge.text}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
