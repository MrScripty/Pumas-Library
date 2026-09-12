import { Play, Square } from 'lucide-react';
import type { ModelInfo } from '../types/apps';
import type { ServedModelStatus } from '../types/api-serving';
import type { LocalModelRowState } from './LocalModelRowState';
import { IconButton } from './ui';

export interface RuntimeModelServeActionProps {
  model: ModelInfo;
  isLoading?: boolean;
  rowState: LocalModelRowState;
  servedStatus?: ServedModelStatus | null;
  onServeModel?: (model: ModelInfo) => void;
}

export function RuntimeModelServeAction({
  model,
  isLoading = false,
  rowState,
  servedStatus,
  onServeModel,
}: RuntimeModelServeActionProps) {
  if (!onServeModel) {
    return null;
  }

  const isLoaded = servedStatus?.load_state === 'loaded';
  return (
    <IconButton
      icon={isLoaded ? <Square /> : <Play />}
      tooltip={isLoaded ? 'Unload model' : isLoading ? 'Loading model' : 'Serve model'}
      onClick={() => onServeModel(model)}
      disabled={rowState.isPartialDownload}
      size="sm"
      active={isLoaded}
      className={
        isLoaded
          ? 'text-[hsl(var(--accent-success))] bg-[hsl(var(--accent-success)/0.12)]'
          : undefined
      }
    />
  );
}
