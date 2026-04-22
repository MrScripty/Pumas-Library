import type { MouseEvent } from 'react';

interface LocalModelNameButtonProps {
  hasIntegrityIssue: boolean;
  integrityIssueMessage?: string | undefined;
  isDownloading: boolean;
  isLinked: boolean;
  isPartialDownload: boolean;
  modelId: string;
  modelName: string;
  onOpenMetadata: (modelId: string, modelName: string) => void;
  wasDequantized?: boolean | undefined;
}

export function LocalModelNameButton({
  hasIntegrityIssue,
  integrityIssueMessage,
  isDownloading,
  isLinked,
  isPartialDownload,
  modelId,
  modelName,
  onOpenMetadata,
  wasDequantized,
}: LocalModelNameButtonProps) {
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      event.stopPropagation();
      onOpenMetadata(modelId, modelName);
    }
  };

  return (
    <button
      type="button"
      className={`text-sm font-medium flex max-w-full items-center text-left bg-transparent border-0 p-0 cursor-pointer ${
        isDownloading
          ? 'text-[hsl(var(--text-muted))]'
          : isPartialDownload
          ? 'text-[hsl(var(--launcher-accent-warning))]'
          : isLinked
          ? 'text-[hsl(var(--text-primary))]'
          : 'text-[hsl(var(--text-secondary))]'
      }`}
      onClick={handleClick}
      title="Ctrl+click to view metadata"
    >
      <span className="truncate">{modelName}</span>
      {wasDequantized && (
        <span
          className="ml-1.5 inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded
            bg-[hsl(var(--launcher-accent-warning)/0.15)]
            text-[hsl(var(--launcher-accent-warning))]"
          title="Dequantized from quantized GGUF - may have reduced precision"
        >
          DQ
        </span>
      )}
      {hasIntegrityIssue && (
        <span
          className="ml-1.5 inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded
            bg-[hsl(var(--accent-warning)/0.2)]
            text-[hsl(var(--accent-warning))]"
          title={integrityIssueMessage ?? 'Library integrity issue detected for this model.'}
        >
          ISSUE
        </span>
      )}
      {isPartialDownload && (
        <span
          className="ml-1.5 inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded
            bg-[hsl(var(--launcher-accent-warning)/0.15)]
            text-[hsl(var(--launcher-accent-warning))]"
          title="Partial download detected - some expected files are missing"
        >
          PARTIAL
        </span>
      )}
    </button>
  );
}
