import { Plus, Trash2 } from 'lucide-react';
import type { InferenceParamSchema } from '../types/api';
import {
  normalizeAllowedOption,
  normalizeStringDefault,
  PARAM_TYPE_OPTIONS,
  type SelectAllowedOption,
} from './ModelMetadataFieldConfig';

interface ModelInferenceSettingsEditorProps {
  addingParam: boolean;
  inferenceSettings: InferenceParamSchema[];
  newParam: {
    key: string;
    label: string;
    param_type: InferenceParamSchema['param_type'];
  };
  saveError: string | null;
  saveSuccess: boolean;
  saving: boolean;
  onAddParam: () => void;
  onNewParamChange: (
    updater: (
      current: {
        key: string;
        label: string;
        param_type: InferenceParamSchema['param_type'];
      }
    ) => {
      key: string;
      label: string;
      param_type: InferenceParamSchema['param_type'];
    }
  ) => void;
  onParamDefaultChange: (index: number, value: string) => void;
  onRemoveParam: (index: number) => void;
  onSave: () => void;
  onSetAddingParam: (next: boolean) => void;
}

export function ModelInferenceSettingsEditor({
  addingParam,
  inferenceSettings,
  newParam,
  saveError,
  saveSuccess,
  saving,
  onAddParam,
  onNewParamChange,
  onParamDefaultChange,
  onRemoveParam,
  onSave,
  onSetAddingParam,
}: ModelInferenceSettingsEditorProps) {
  return (
    <div className="space-y-4 max-h-80 overflow-y-auto">
      {inferenceSettings.length === 0 ? (
        <div className="text-center py-4 text-[hsl(var(--text-muted))] text-sm">
          No inference settings configured for this model.
        </div>
      ) : (
        <div className="space-y-2">
          {inferenceSettings.map((param, index) => {
            const allowedOptions = (param.constraints?.allowed_values ?? [])
              .map(normalizeAllowedOption)
              .filter((option): option is SelectAllowedOption => option !== null);

            return (
              <div key={param.key} className="flex items-center gap-2">
                <div className="flex-1 min-w-0">
                  <label
                    className="block text-xs text-[hsl(var(--text-muted))] truncate"
                    title={param.description || param.key}
                  >
                    {param.label}
                    <span className="ml-1 opacity-50">({param.param_type})</span>
                  </label>
                  {param.param_type === 'Boolean' ? (
                    <select
                      value={String(param.default)}
                      onChange={(event) => onParamDefaultChange(index, event.target.value)}
                      className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                    >
                      <option value="true">true</option>
                      <option value="false">false</option>
                    </select>
                  ) : param.param_type === 'String' && allowedOptions.length > 0 ? (
                    <select
                      value={normalizeStringDefault(param.default)}
                      onChange={(event) => onParamDefaultChange(index, event.target.value)}
                      className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                    >
                      {allowedOptions.map((option) => (
                        <option
                          key={`${option.label}:${option.value}`}
                          value={option.value}
                        >
                          {option.label}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <input
                      type={param.param_type === 'String' ? 'text' : 'number'}
                      value={param.default == null ? '' : String(param.default)}
                      onChange={(event) => onParamDefaultChange(index, event.target.value)}
                      placeholder={param.description || param.key}
                      min={param.constraints?.min ?? undefined}
                      max={param.constraints?.max ?? undefined}
                      step={param.param_type === 'Integer' ? 1 : 'any'}
                      className="w-full px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
                    />
                  )}
                </div>
                <button
                  onClick={() => onRemoveParam(index)}
                  className="p-1 mt-4 text-[hsl(var(--text-muted))] hover:text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error)/0.1)] rounded"
                  title="Remove parameter"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            );
          })}
        </div>
      )}

      {addingParam ? (
        <div className="space-y-2 p-3 bg-[hsl(var(--surface-high)/0.5)] rounded border border-[hsl(var(--border-default))]">
          <div className="grid grid-cols-3 gap-2">
            <input
              type="text"
              value={newParam.key}
              onChange={(event) =>
                onNewParamChange((current) => ({ ...current, key: event.target.value }))
              }
              placeholder="key"
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            />
            <input
              type="text"
              value={newParam.label}
              onChange={(event) =>
                onNewParamChange((current) => ({ ...current, label: event.target.value }))
              }
              placeholder="Label"
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            />
            <select
              value={newParam.param_type}
              onChange={(event) =>
                onNewParamChange((current) => ({
                  ...current,
                  param_type: event.target.value as InferenceParamSchema['param_type'],
                }))
              }
              className="px-2 py-1 text-sm bg-[hsl(var(--surface-high))] border border-[hsl(var(--border-default))] rounded text-[hsl(var(--text-primary))]"
            >
              {PARAM_TYPE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
          <div className="flex gap-2">
            <button
              onClick={onAddParam}
              disabled={!newParam.key.trim() || !newParam.label.trim()}
              className="px-3 py-1 text-xs bg-[hsl(var(--launcher-accent-primary))] text-white rounded hover:opacity-90 disabled:opacity-40"
            >
              Add
            </button>
            <button
              onClick={() => onSetAddingParam(false)}
              className="px-3 py-1 text-xs bg-[hsl(var(--surface-high))] text-[hsl(var(--text-secondary))] rounded hover:bg-[hsl(var(--surface-mid))]"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => onSetAddingParam(true)}
          className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))] hover:text-[hsl(var(--text-primary))]"
        >
          <Plus className="w-3 h-3" /> Add Parameter
        </button>
      )}

      <div className="flex items-center gap-3 pt-2 border-t border-[hsl(var(--border-default))]">
        <button
          onClick={onSave}
          disabled={saving}
          className="px-4 py-1.5 text-sm bg-[hsl(var(--launcher-accent-primary))] text-white rounded hover:opacity-90 disabled:opacity-50"
        >
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
        {saveSuccess && (
          <span className="text-xs text-[hsl(var(--accent-success))]">Saved</span>
        )}
        {saveError && (
          <span className="text-xs text-[hsl(var(--accent-error))]">{saveError}</span>
        )}
      </div>
    </div>
  );
}
