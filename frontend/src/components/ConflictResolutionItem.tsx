import { AnimatePresence, motion } from 'framer-motion';
import {
  ChevronDown,
  ChevronUp,
  Edit3,
  FileSymlink,
  Replace,
  SkipForward,
  type LucideIcon,
} from 'lucide-react';
import type { MappingAction } from '../types/api';
import type { ConflictResolutionAction } from '../hooks/useConflictResolutions';

export const conflictActionOptions: Array<{
  value: ConflictResolutionAction;
  label: string;
  description: string;
  icon: LucideIcon;
  color: string;
}> = [
  {
    value: 'skip',
    label: 'Skip',
    description: 'Keep existing file, do not create link',
    icon: SkipForward,
    color: 'text-[hsl(var(--launcher-text-tertiary))]',
  },
  {
    value: 'overwrite',
    label: 'Overwrite',
    description: 'Replace existing with new symlink',
    icon: Replace,
    color: 'text-[hsl(var(--accent-warning))]',
  },
  {
    value: 'rename',
    label: 'Rename Existing',
    description: 'Rename existing to .old, create new link',
    icon: Edit3,
    color: 'text-[hsl(var(--accent-primary))]',
  },
];

function getConflictDescription(reason: string): string {
  if (reason.includes('different source')) {
    return 'Symlink points to a different model file';
  }
  if (reason.includes('file exists') || reason.includes('Non-symlink')) {
    return 'A regular file exists at this location';
  }
  return reason;
}

interface ConflictResolutionItemProps {
  conflict: MappingAction;
  currentResolution: ConflictResolutionAction;
  isApplying: boolean;
  isExpanded: boolean;
  onResolutionChange: (modelId: string, action: ConflictResolutionAction) => void;
  onToggleExpanded: (modelId: string) => void;
}

export function ConflictResolutionItem({
  conflict,
  currentResolution,
  isApplying,
  isExpanded,
  onResolutionChange,
  onToggleExpanded,
}: ConflictResolutionItemProps) {
  return (
    <div className="border border-[hsl(var(--launcher-border)/0.5)] rounded-lg overflow-hidden">
      <button
        onClick={() => onToggleExpanded(conflict.model_id)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.3)] transition-colors"
      >
        <div className="flex items-center gap-3 flex-1 min-w-0">
          <FileSymlink className="w-4 h-4 text-[hsl(var(--accent-warning))] flex-shrink-0" />
          <div className="flex-1 min-w-0 text-left">
            <div className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
              {conflict.model_name || conflict.model_id}
            </div>
            <div className="text-xs text-[hsl(var(--accent-warning))]">
              {getConflictDescription(conflict.reason)}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3 flex-shrink-0">
          <select
            value={currentResolution}
            onChange={(e) => {
              e.stopPropagation();
              onResolutionChange(
                conflict.model_id,
                e.target.value as ConflictResolutionAction
              );
            }}
            onClick={(e) => e.stopPropagation()}
            disabled={isApplying}
            className="px-3 py-1.5 text-xs rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))] text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
          >
            {conflictActionOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          ) : (
            <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          )}
        </div>
      </button>

      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-3 pt-1 space-y-2 bg-[hsl(var(--launcher-bg-secondary)/0.3)]">
              <div className="text-xs space-y-1">
                <div className="flex">
                  <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Source:</span>
                  <span className="text-[hsl(var(--launcher-text-secondary))] font-mono truncate flex-1">
                    {conflict.source_path.split('/').slice(-2).join('/')}
                  </span>
                </div>
                <div className="flex">
                  <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Target:</span>
                  <span className="text-[hsl(var(--launcher-text-secondary))] font-mono truncate flex-1">
                    {conflict.target_path.split('/').slice(-2).join('/')}
                  </span>
                </div>
                {conflict.existing_target && (
                  <div className="flex">
                    <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Existing:</span>
                    <span className="text-[hsl(var(--accent-warning))] font-mono truncate flex-1">
                      {conflict.existing_target}
                    </span>
                  </div>
                )}
              </div>

              <div className="pt-2 border-t border-[hsl(var(--launcher-border)/0.3)]">
                {conflictActionOptions.map((option) => (
                  // eslint-disable-next-line jsx-a11y/label-has-associated-control
                  <label
                    key={option.value}
                    htmlFor={`resolution-${conflict.model_id}-${option.value}`}
                    className={`flex items-start gap-2 py-1 cursor-pointer ${
                      currentResolution === option.value
                        ? 'opacity-100'
                        : 'opacity-50 hover:opacity-75'
                    }`}
                  >
                    <input
                      id={`resolution-${conflict.model_id}-${option.value}`}
                      type="radio"
                      name={`resolution-${conflict.model_id}`}
                      value={option.value}
                      checked={currentResolution === option.value}
                      onChange={() => onResolutionChange(conflict.model_id, option.value)}
                      disabled={isApplying}
                      className="mt-0.5"
                    />
                    <div className="flex-1">
                      <div className={`text-xs font-medium ${option.color}`}>
                        {option.label}
                      </div>
                      <div className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
                        {option.description}
                      </div>
                    </div>
                  </label>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
