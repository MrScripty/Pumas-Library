/**
 * Choose what a model is used for. The choice is library metadata: the model
 * keeps its folder and ID, and later re-detection keeps the choice until it
 * is cleared.
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
  onChanged: (category: string, resolutionSource: string | null) => void;
}

export function ModelCategoryControl({
  modelId,
  category,
  resolutionSource,
  onChanged,
}: ModelCategoryControlProps) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const chosen = resolutionSource === USER_CATEGORY_SOURCE;

  const choose = async (value: string) => {
    setSaving(true);
    setError(null);
    try {
      const result = await modelsAPI.setModelCategory(modelId, value === DETECT ? null : value);
      onChanged(result.category, value === DETECT ? null : USER_CATEGORY_SOURCE);
    } catch (cause) {
      setError(cause instanceof Error && cause.message ? cause.message : 'The category could not be saved.');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="mb-3 flex flex-wrap items-center gap-2 text-xs text-[hsl(var(--text-secondary))]">
      <label htmlFor={`model-category-${modelId}`} className="font-medium">
        Used as
      </label>
      <select
        id={`model-category-${modelId}`}
        className="rounded border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-control))] px-2 py-1 text-[hsl(var(--text-primary))]"
        disabled={saving}
        value={chosen ? category : DETECT}
        onChange={(event) => void choose(event.target.value)}
      >
        <option value={DETECT}>
          Detected: {modelCategoryLabel(category)}
        </option>
        {CHOOSABLE_MODEL_CATEGORIES.map((option) => (
          <option key={option} value={option}>
            {modelCategoryLabel(option)}
          </option>
        ))}
      </select>
      <span className="text-[hsl(var(--text-muted))]">
        {chosen ? 'Chosen by you; re-scans keep it.' : 'Detected from the model files and source.'}
      </span>
      {error && (
        <span role="alert" className="w-full text-[hsl(var(--accent-error))]">
          {error}
        </span>
      )}
    </div>
  );
}
