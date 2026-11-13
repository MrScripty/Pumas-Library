/**
 * Library-wide backfill of model cards, licenses and code. The backend plans,
 * fetches, verifies and explains each skip; this panel shows a dry run first
 * and fetches only after the user asks.
 */

import { useState } from 'react';
import { ChevronDown, ChevronUp, Loader2, ScrollText } from 'lucide-react';
import { isAPIAvailable } from '../api/adapter';
import { modelsAPI } from '../api/models';
import type { CompanionBackfillReport } from '../types/api';
import { formatBytes } from '../utils/formatters';

function countBy(report: CompanionBackfillReport, key: 'outcome' | 'reason'): [string, number][] {
  const counts = new Map<string, number>();
  for (const model of report.models) {
    const value = model[key];
    if (value) counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

export function LibraryCompanionBackfillPanel() {
  const [expanded, setExpanded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<CompanionBackfillReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!isAPIAvailable()) return null;

  const run = async (dryRun: boolean) => {
    setBusy(true);
    setError(null);
    try {
      setReport(await modelsAPI.backfillLibraryCompanionFiles(dryRun));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const canFetch = report?.dry_run === true && report.file_count > 0;

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      <button
        onClick={() => setExpanded((previous) => !previous)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
      >
        <div className="flex items-center gap-3">
          <ScrollText className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            Model Cards, Licenses &amp; Code
          </span>
          {busy && <Loader2 className="w-4 h-4 animate-spin text-[hsl(var(--launcher-text-secondary))]" />}
        </div>
        {expanded ? (
          <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
        ) : (
          <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
        )}
      </button>
      {expanded && (
        <div className="px-4 pb-4 space-y-3 text-sm">
          <p className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
            Fetch the model card, license, code and metadata files that downloaded models are
            missing, at each model&apos;s recorded revision. Weights and model identity are never
            changed, and stored code is never executed.
          </p>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => void run(true)}
              className="px-3 py-1.5 rounded text-sm bg-[hsl(var(--launcher-bg-tertiary))]"
            >
              Check library
            </button>
            {canFetch && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void run(false)}
                className="px-3 py-1.5 rounded text-sm bg-[hsl(var(--launcher-accent-primary)/0.2)]"
              >
                Fetch {report.file_count} file(s)
              </button>
            )}
          </div>
          {error && (
            <div role="alert" className="text-xs text-[hsl(var(--accent-error))]">
              {error}
            </div>
          )}
          {report && (
            <div role="status" className="text-xs text-[hsl(var(--launcher-text-secondary))] space-y-1">
              <div>
                {report.dry_run ? 'Would fetch' : 'Fetched'} {report.file_count} file(s),{' '}
                {formatBytes(report.total_bytes)}, across {report.models.length} model(s).
              </div>
              <div>
                {countBy(report, 'outcome')
                  .map(([outcome, count]) => `${outcome.replace(/_/g, ' ')}: ${count}`)
                  .join(' · ')}
              </div>
              {countBy(report, 'reason').length > 0 && (
                <div>
                  Reasons:{' '}
                  {countBy(report, 'reason')
                    .map(([reason, count]) => `${reason.replace(/_/g, ' ')} (${count})`)
                    .join(', ')}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
