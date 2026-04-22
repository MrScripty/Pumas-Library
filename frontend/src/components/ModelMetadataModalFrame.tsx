import { useEffect, useId, useRef, type ReactNode } from 'react';
import { RefreshCw, X } from 'lucide-react';

interface ModelMetadataModalFrameProps {
  children: ReactNode;
  isLoading: boolean;
  isRefetching: boolean;
  modelName: string;
  onClose: () => void;
  onRefetch: () => void;
}

export function ModelMetadataModalFrame({
  children,
  isLoading,
  isRefetching,
  modelName,
  onClose,
  onRefetch,
}: ModelMetadataModalFrameProps) {
  const titleId = useId();
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const focusTimer = window.setTimeout(() => closeButtonRef.current?.focus(), 0);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener('keydown', handleKeyDown);
      previousFocus?.focus();
    };
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none">
      <button
        type="button"
        className="absolute inset-0 bg-black/50 pointer-events-auto"
        aria-label="Close metadata modal"
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="relative pointer-events-auto bg-[hsl(var(--surface-overlay)/0.95)] border border-[hsl(var(--border-default))] backdrop-blur-md rounded-lg shadow-lg w-full max-w-2xl max-h-[80vh] overflow-hidden"
      >
        <div className="flex items-center justify-between p-4 border-b border-[hsl(var(--border-default))]">
          <h2 id={titleId} className="text-lg font-semibold truncate text-[hsl(var(--text-primary))]">
            {modelName}
          </h2>
          <div className="flex items-center gap-1">
            <button
              onClick={onRefetch}
              disabled={isRefetching || isLoading}
              className="p-1 hover:bg-[hsl(var(--surface-mid))] rounded text-[hsl(var(--text-secondary))] disabled:opacity-40"
              aria-label="Refetch metadata from HuggingFace"
              title="Refetch metadata from HuggingFace"
            >
              <RefreshCw className={`w-4 h-4 ${isRefetching ? 'animate-spin' : ''}`} />
            </button>
            <button
              ref={closeButtonRef}
              onClick={onClose}
              className="p-1 hover:bg-[hsl(var(--surface-mid))] rounded text-[hsl(var(--text-secondary))]"
              aria-label="Close"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <div className="p-4 overflow-y-auto">
          {children}
        </div>
      </div>
    </div>
  );
}
