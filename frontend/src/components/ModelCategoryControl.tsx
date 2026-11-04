/**
 * Choose what a model is used for. The choice is library metadata: the model
 * keeps its folder and ID, and later re-detection keeps the choice until it
 * is cleared.
 *
 * Picking an option does not save it; **Apply** does, so stepping through the
 * options with the keyboard records nothing. The backend owns the result: after
 * a save the parent re-reads the model's metadata, and the picker follows it
 * unless the person has picked something else in the meantime.
 */

import { useState } from 'react';
import { modelsAPI } from '../api/models';
import {
  CHOOSABLE_MODEL_CATEGORIES,
  modelCategoryLabel,
} from '../utils/modelCategories';

/** `model_type_resolution_source` recorded when a person chose the category. */
const USER_CATEGORY_SOURCE = 'user';
const DETECT = '';

interface ModelCategoryControlProps {
  modelId: string;
  category: string;
  resolutionSource: string | null;
  /** Called after the backend saved a change; re-read the model's metadata. */
  onChanged: () => void;
}

export function ModelCategoryControl({
  modelId,
  category,
  resolutionSource,
  onChanged,
}: ModelCategoryControlProps) {
  const chosen = resolutionSource === USER_CATEGORY_SOURCE;
  const current = chosen ? category : DETECT;
  const [pending, setPending] = useState(current);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [synced, setSynced] = useState(current);
  const [applied, setApplied] = useState<string | null>(null);

  // The saved category arrives after Apply, from a separate metadata read.
  // Follow it only when the picker still shows the previous or the applied
  // value; a newer pick made while the read was in flight is kept.
  if (current !== synced) {
    setSynced(current);
    setApplied(null);
    if (pending === synced || pending === applied) {
      setPending(current);
    }
  }

  const apply = async () => {
    setSaving(true);
    setError(null);
    setSaved(null);
    setApplied(pending);
    try {
      const result = await modelsAPI.setModelCategory(modelId, pending === DETECT ? null : pending);
      setSaved(
        pending === DETECT
          ? `Detected as ${modelCategoryLabel(result.category)}.`
          : `Saved as ${modelCategoryLabel(result.category)}.`,
      );
      onChanged();
    } catch (cause) {
      setError(cause instanceof Error && cause.message ? cause.message : 'The category could not be saved.');
    } finally {
      setSaving(false);
    }
  };

  const selectId = `model-category-${modelId}`;
  const hintId = `${selectId}-hint`;
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2 text-xs text-[hsl(var(--text-secondary))]">
      <label htmlFor={selectId} className="font-medium">
        Used as
      </label>
      <select
        id={selectId}
        aria-describedby={hintId}
        className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-control))] px-2 py-1 text-[hsl(var(--text-primary))]"
        value={pending}
        onChange={(event) => setPending(event.target.value)}
      >
        <option value={DETECT}>Detect automatically</option>
        {CHOOSABLE_MODEL_CATEGORIES.map((option) => (
          <option key={option} value={option}>
            {modelCategoryLabel(option)}
          </option>
        ))}
      </select>
      <button
        type="button"
        className="rounded border border-[hsl(var(--border-default))] px-2 py-1 text-[hsl(var(--text-primary))] disabled:opacity-50"
        disabled={saving || pending === current}
        onClick={() => void apply()}
      >
        {saving ? 'Saving…' : 'Apply'}
      </button>
      <span id={hintId} className="text-[hsl(var(--text-muted))]">
        {chosen
          ? `Chosen by you: ${modelCategoryLabel(category)}. Re-scans keep it.`
          : `Detected as ${modelCategoryLabel(category)} from the model files and source.`}
      </span>
      <span role="status" className="sr-only">
        {saved}
      </span>
      {error && (
        <span role="alert" className="w-full text-[hsl(var(--accent-error))]">
          {error}
        </span>
      )}
    </div>
  );
}
