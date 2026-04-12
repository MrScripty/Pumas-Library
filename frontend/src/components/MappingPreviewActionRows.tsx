import { Link } from 'lucide-react';
import type { MappingAction } from './MappingPreviewTypes';

export function renderCreateAction(action: MappingAction, index: number) {
  return (
    <div
      key={index}
      className="text-xs p-2 bg-[hsl(var(--accent-success)/0.05)] rounded"
    >
      <div className="font-medium text-[hsl(var(--launcher-text-primary))] truncate">
        {action.model_name || action.model_id}
      </div>
      <div className="flex items-center gap-1 text-[hsl(var(--launcher-text-tertiary))] mt-1">
        <Link className="w-3 h-3" />
        <span className="truncate font-mono">
          {action.target_path.split('/').slice(-2).join('/')}
        </span>
      </div>
    </div>
  );
}

export function renderSkipAction(action: MappingAction, index: number) {
  return (
    <div
      key={index}
      className="text-xs p-2 bg-[hsl(var(--launcher-bg-secondary)/0.3)] rounded font-mono truncate text-[hsl(var(--launcher-text-tertiary))]"
    >
      {action.target_path.split('/').pop()}
    </div>
  );
}

export function renderConflictAction(action: MappingAction, index: number) {
  return (
    <div
      key={index}
      className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)]"
    >
      <div className="font-medium text-[hsl(var(--launcher-text-primary))] truncate">
        {action.model_name || action.model_id}
      </div>
      <div className="text-[hsl(var(--accent-warning))] mt-1">{action.reason}</div>
      {action.existing_target && (
        <div className="font-mono text-[hsl(var(--launcher-text-tertiary))] mt-1 truncate">
          {'->'} {action.existing_target}
        </div>
      )}
    </div>
  );
}
