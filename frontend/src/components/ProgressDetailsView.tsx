/**
 * Progress Details View Component
 *
 * Detailed installation progress display with stages, stats, and completed items.
 * Extracted from InstallDialog.tsx
 */

import { motion, AnimatePresence } from 'framer-motion';
import {
  Download,
  Package,
  FolderArchive,
  Settings,
  CheckCircle2,
  Clock,
  ArrowLeft,
  ChevronDown,
  ChevronUp,
  AlertCircle,
  FileText,
} from 'lucide-react';
import type { InstallationProgress } from '../hooks/useVersions';
import { formatBytes, formatSpeed } from '../utils/formatters';
import { formatElapsedTime } from '../utils/installationFormatters';
import { IconButton } from './ui';
import { getInstallActivityPresentation } from '../utils/installActivityPresentation';

const STAGE_LABELS = {
  download: 'Downloading',
  extract: 'Extracting',
  venv: 'Creating Environment',
  dependencies: 'Installing Dependencies',
  setup: 'Final Setup'
};

const STAGE_ICONS = {
  download: Download,
  extract: FolderArchive,
  venv: Settings,
  dependencies: Package,
  setup: CheckCircle2
};

type InstallationOutcome = {
  kind: 'cancelled' | 'failed' | 'succeeded';
  message: string;
  title: string;
};

function clampProgress(value: number): number {
  return Math.min(100, Math.max(0, value));
}

function getInstallationOutcome(
  progress: InstallationProgress,
  installingVersion: string | null
): InstallationOutcome | null {
  if (progress.error) {
    if (progress.error.toLowerCase().includes('cancel')) {
      return {
        kind: 'cancelled',
        title: 'Installation Cancelled',
        message: 'The installation was stopped and incomplete files have been removed',
      };
    }
    return {
      kind: 'failed',
      title: 'Installation Failed',
      message: progress.error,
    };
  }

  if (progress.completed_at && progress.success) {
    return {
      kind: 'succeeded',
      title: 'Installation Complete!',
      message: `${installingVersion ?? progress.tag} has been successfully installed`,
    };
  }

  if (progress.completed_at && progress.success === false) {
    return {
      kind: 'failed',
      title: 'Installation Failed',
      message: 'The installation did not complete successfully',
    };
  }

  return null;
}

interface ProgressDetailsViewProps {
  appId?: string;
  progress: InstallationProgress;
  installingVersion: string | null;
  showCompletedItems: boolean;
  onToggleCompletedItems: () => void;
  onBackToList: () => void;
  onOpenLogPath: (path?: string | null) => void;
}

export function ProgressDetailsView({
  appId,
  progress,
  installingVersion,
  showCompletedItems,
  onToggleCompletedItems,
  onBackToList,
  onOpenLogPath,
}: ProgressDetailsViewProps) {
  const CurrentStageIcon = STAGE_ICONS[progress.stage];
  const overallProgress = clampProgress(progress.overall_progress);
  const stageProgress = clampProgress(progress.stage_progress);
  const activity = getInstallActivityPresentation({ appId, installingTag: installingVersion, progress });
  const outcome = getInstallationOutcome(progress, installingVersion);
  const OutcomeIcon = outcome?.kind === 'succeeded' ? CheckCircle2 : AlertCircle;
  const outcomeContainerClass = outcome?.kind === 'succeeded'
    ? 'bg-[hsl(var(--accent-success))]/10 border-[hsl(var(--accent-success))]/30'
    : outcome?.kind === 'cancelled'
      ? 'bg-[hsl(var(--accent-warning))]/10 border-[hsl(var(--accent-warning))]/30'
      : 'bg-[hsl(var(--accent-error))]/10 border-[hsl(var(--accent-error))]/30';
  const outcomeTextClass = outcome?.kind === 'succeeded'
    ? 'text-[hsl(var(--accent-success))]'
    : outcome?.kind === 'cancelled'
      ? 'text-[hsl(var(--accent-warning))]'
      : 'text-[hsl(var(--accent-error))]';

  return (
    <div className="space-y-3 px-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <IconButton
          icon={<ArrowLeft />}
          tooltip="Back"
          onClick={onBackToList}
          size="md"
        />
        <span className="text-sm text-[hsl(var(--text-muted))] truncate">
          {installingVersion ? `Installing ${installingVersion}` : 'Installation details'}
        </span>
      </div>

      {/* Overall Progress */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-medium text-[hsl(var(--text-secondary))]">Overall Progress</span>
          <span className="text-sm font-semibold text-[hsl(var(--text-primary))]">{activity.indeterminate ? 'Working…' : `${overallProgress}%`}</span>
        </div>
        <div
          role="progressbar"
          aria-label="Overall installation progress"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={activity.indeterminate ? undefined : overallProgress}
          className="w-full h-2 bg-[hsl(var(--surface-low))] rounded-full overflow-hidden"
        >
          {activity.indeterminate ? <div className="h-full w-1/3 animate-pulse rounded-full bg-[hsl(var(--accent-success))]" /> : <motion.div
            className="h-full bg-[hsl(var(--accent-success))] rounded-full"
            initial={{ width: 0 }}
            animate={{ width: `${overallProgress}%` }}
            transition={{ duration: 0.3 }}
          />}
        </div>
      </div>

      {/* Current Stage */}
      <div className="bg-[hsl(var(--surface-low))] rounded-lg p-4">
        <div className="flex items-start gap-3">
          <div className="p-2 bg-[hsl(var(--accent-success))]/10 rounded-lg">
            <CurrentStageIcon size={24} className="text-[hsl(var(--accent-success))]" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center justify-between mb-1">
              <h3 className="text-[hsl(var(--text-primary))] font-medium">
                {STAGE_LABELS[progress.stage]}
              </h3>
              <span className="text-sm text-[hsl(var(--text-muted))]">
                {activity.indeterminate ? 'In progress' : `${stageProgress}%`}
              </span>
            </div>
            {progress.current_item && (
              <p className="text-sm text-[hsl(var(--text-muted))] truncate">
                {progress.current_item}
              </p>
            )}
            <div
              role="progressbar"
              aria-label={`${STAGE_LABELS[progress.stage]} progress`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={activity.indeterminate ? undefined : stageProgress}
              className="w-full h-1.5 bg-[hsl(var(--surface-lowest))] rounded-full overflow-hidden mt-2"
            >
              {activity.indeterminate ? <div className="h-full w-1/3 animate-pulse rounded-full bg-[hsl(var(--accent-success))]/50" /> : <motion.div
                className="h-full bg-[hsl(var(--accent-success))]/50 rounded-full"
                initial={{ width: 0 }}
                animate={{ width: `${stageProgress}%` }}
                transition={{ duration: 0.3 }}
              />}
            </div>
          </div>
        </div>
      </div>

      {/* Stage-specific Stats */}
      {progress.download_speed !== null && (
        <div className="bg-[hsl(var(--surface-low))] rounded-lg p-3">
          <div className="flex items-center gap-2 mb-1">
            <Download size={14} className="text-[hsl(var(--text-muted))]" />
            <span className="text-xs text-[hsl(var(--text-muted))]">Speed</span>
          </div>
          <span className="text-base font-semibold text-[hsl(var(--text-primary))]">
            {formatSpeed(progress.download_speed)}
          </span>
        </div>
      )}

      {progress.stage === 'dependencies' && progress.dependency_count !== null && (
        <div className="bg-[hsl(var(--surface-low))] rounded-lg p-3">
          <div className="flex items-center gap-2 mb-1">
            <Package size={14} className="text-[hsl(var(--text-muted))]" />
            <span className="text-xs text-[hsl(var(--text-muted))]">Packages</span>
          </div>
          <span className="text-base font-semibold text-[hsl(var(--text-primary))]">
            {progress.completed_dependencies} / {progress.dependency_count}
          </span>
        </div>
      )}

      {/* Elapsed Time */}
      <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-muted))]">
        <Clock size={14} />
        <span>Elapsed: {formatElapsedTime(progress.started_at)}</span>
      </div>

      {/* Expandable Completed Items */}
      {progress.completed_items.length > 0 && (
        <div className="bg-[hsl(var(--surface-low))] rounded-lg overflow-hidden">
          <button
            onClick={onToggleCompletedItems}
            className="w-full flex items-center justify-between p-3 hover:bg-[hsl(var(--surface-tertiary))] transition-colors"
          >
            <div className="flex items-center gap-2">
              <CheckCircle2 size={14} className="text-[hsl(var(--accent-success))]" />
              <span className="text-sm font-medium text-[hsl(var(--text-primary))]">
                Completed Items ({progress.completed_items.length})
              </span>
            </div>
            {showCompletedItems ? (
              <ChevronUp size={14} className="text-[hsl(var(--text-muted))]" />
            ) : (
              <ChevronDown size={14} className="text-[hsl(var(--text-muted))]" />
            )}
          </button>
          <AnimatePresence>
            {showCompletedItems && (
              <motion.div
                initial={{ height: 0 }}
                animate={{ height: 'auto' }}
                exit={{ height: 0 }}
                className="overflow-hidden"
              >
                <div className="max-h-40 overflow-y-auto p-3 pt-0 space-y-1">
                  {progress.completed_items.map((item, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between text-xs py-1 px-2 rounded hover:bg-[hsl(var(--surface-lowest))]"
                    >
                      <span className="text-[hsl(var(--text-secondary))] truncate flex-1">
                        {item.name}
                      </span>
                      {item.size !== null && (
                        <span className="text-[hsl(var(--text-muted))] text-xs ml-2">
                          {formatBytes(item.size)}
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Terminal outcome only; progress updates stay outside the live region. */}
      {outcome && (
        <div
          role={outcome.kind === 'failed' ? 'alert' : 'status'}
          aria-live={outcome.kind === 'failed' ? 'assertive' : 'polite'}
          aria-atomic="true"
          className={`${outcomeContainerClass} flex items-center gap-3 rounded-lg border p-3`}
        >
          <OutcomeIcon size={20} className={outcomeTextClass} />
          <div>
            <p className={`${outcomeTextClass} text-sm font-medium`}>{outcome.title}</p>
            <p className="mt-1 text-xs text-[hsl(var(--text-muted))]">{outcome.message}</p>
            {outcome.kind === 'failed' && progress.log_path && (
              <button
                type="button"
                onClick={() => onOpenLogPath(progress.log_path)}
                className="mt-2 inline-flex items-center gap-2 px-2 py-1 rounded border border-[hsl(var(--accent-error))]/40 text-xs text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error))]/10"
              >
                <FileText size={12} />
                <span>Open log</span>
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
