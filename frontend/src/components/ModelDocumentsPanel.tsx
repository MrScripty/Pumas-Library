/**
 * Shows the model card, license documents and code the backend stores with a
 * library model, and lets the user fetch the ones it is missing. The backend
 * decides what is stored, what a backfill fetches, and why it skips a model;
 * this panel only renders those results.
 */

import { useCallback, useEffect, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { modelsAPI } from '../api/models';
import type { CompanionBackfillReport, ModelDocumentsResponse } from '../types/api';
import { formatBytes } from '../utils/formatters';
import { ModelNotesMarkdownPreview } from './ModelNotesMarkdownPreview';

/** Card front matter is shown as stored metadata; render the prose only. */
function withoutFrontMatter(markdown: string): string {
  const match = /^---\r?\n[\s\S]*?\r?\n---\r?\n?/.exec(markdown);
  return match ? markdown.slice(match[0].length) : markdown;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function BackfillSummary({ report }: { report: CompanionBackfillReport }) {
  const model = report.models[0];
  if (!model) return null;
  if (model.outcome === 'skipped' || model.outcome === 'failed') {
    return (
      <div role="status" className="text-xs text-[hsl(var(--text-muted))]">
        {model.outcome === 'failed' ? 'Fetch failed' : 'Nothing to fetch'}: {model.message ?? model.reason}
      </div>
    );
  }
  if (model.files.length === 0) {
    return (
      <div role="status" className="text-xs text-[hsl(var(--text-muted))]">
        Every card, license and code file is already stored.
      </div>
    );
  }
  return (
    <div role="status" className="text-xs text-[hsl(var(--text-secondary))] space-y-1">
      <div>
        {report.dry_run ? 'Would fetch' : 'Fetched'} {model.files.length} file(s),{' '}
        {formatBytes(report.total_bytes)}, from revision {model.revision?.slice(0, 12)}:
      </div>
      <ul className="font-mono list-disc pl-5">
        {model.files.map((file) => (
          <li key={file.path}>
            {file.path} ({file.kind}, {formatBytes(file.size_bytes)})
          </li>
        ))}
      </ul>
    </div>
  );
}

export function ModelDocumentsPanel({ modelId }: { modelId: string }) {
  const [documents, setDocuments] = useState<ModelDocumentsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [plan, setPlan] = useState<CompanionBackfillReport | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setDocuments(await modelsAPI.getModelDocuments(modelId));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }, [modelId]);

  useEffect(() => {
    void load();
  }, [load]);

  const backfill = async (dryRun: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const report = await modelsAPI.backfillModelCompanionFiles(modelId, dryRun);
      setPlan(report);
      if (!dryRun) await load();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  if (loading) {
    return (
      <div className="flex justify-center py-4">
        <Loader2 className="w-5 h-5 animate-spin text-[hsl(var(--text-muted))]" />
      </div>
    );
  }

  const planned = plan?.dry_run && plan.models[0]?.outcome === 'planned';

  return (
    <div className="space-y-4">
      {error && (
        <div role="alert" className="text-xs text-[hsl(var(--accent-error))]">
          {error}
        </div>
      )}

      {documents?.backfill_available && (
        <div className="space-y-2">
          <div className="flex gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => void backfill(true)}
              className="px-3 py-1.5 rounded text-sm bg-[hsl(var(--surface-high))] hover:bg-[hsl(var(--surface-mid))]"
            >
              Check for missing card, license and code
            </button>
            {planned && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void backfill(false)}
                className="px-3 py-1.5 rounded text-sm bg-[hsl(var(--launcher-accent-primary)/0.2)]"
              >
                Fetch them
              </button>
            )}
          </div>
          {plan && <BackfillSummary report={plan} />}
        </div>
      )}

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-[hsl(var(--text-primary))]">Model card</h3>
        {documents?.model_card ? (
          <div className="text-sm">
            <div className="text-xs font-mono text-[hsl(var(--text-muted))]">
              {documents.model_card.path}
            </div>
            <ModelNotesMarkdownPreview markdown={withoutFrontMatter(documents.model_card.content)} />
          </div>
        ) : (
          <div className="text-sm text-[hsl(var(--text-muted))]">No model card is stored.</div>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-[hsl(var(--text-primary))]">License</h3>
        {documents && documents.licenses.length > 0 ? (
          documents.licenses.map((license) => (
            <details key={license.path} open={license === documents.licenses[0]}>
              <summary className="text-xs font-mono cursor-pointer text-[hsl(var(--text-secondary))]">
                {license.path}
              </summary>
              <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap text-xs text-[hsl(var(--text-secondary))]">
                {license.content}
              </pre>
            </details>
          ))
        ) : (
          <div className="text-sm text-[hsl(var(--text-muted))]">No license file is stored.</div>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-[hsl(var(--text-primary))]">Code</h3>
        <p className="text-xs text-[hsl(var(--text-muted))]">
          Stored for inspection only. Pumas never executes model repository code.
          {documents?.requires_custom_code &&
            ' This model’s config names custom code; runtimes that would need it refuse to load the model.'}
        </p>
        {documents && documents.code_files.length > 0 ? (
          <ul className="text-xs font-mono list-disc pl-5 text-[hsl(var(--text-secondary))]">
            {documents.code_files.map((file) => (
              <li key={file.path}>
                {file.path} ({formatBytes(file.size_bytes)})
              </li>
            ))}
          </ul>
        ) : (
          <div className="text-sm text-[hsl(var(--text-muted))]">No code files are stored.</div>
        )}
      </section>
    </div>
  );
}
