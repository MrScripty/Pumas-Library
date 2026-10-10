import { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { HeaderWindowControls } from './HeaderControls';
import { IconButton } from './ui';

interface RecoveryAction {
  label: string;
  onAction: () => void;
}

export interface LauncherRootRecoveryViewProps {
  title: string;
  message: string;
  primaryAction?: RecoveryAction;
  secondaryAction?: RecoveryAction;
  onClose: () => void;
  onMinimize?: () => void;
}

export function LauncherRootRecoveryView({
  title,
  message,
  primaryAction,
  secondaryAction,
  onClose,
  onMinimize,
}: LauncherRootRecoveryViewProps) {
  const headingRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    headingRef.current?.focus();
  }, [title]);

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden bg-[hsl(var(--surface-lowest))] text-[hsl(var(--text-primary))]">
      <header className="app-region-drag flex h-10 flex-shrink-0 items-center justify-end px-2">
        {onMinimize ? (
          <HeaderWindowControls onClose={onClose} onMinimize={onMinimize} />
        ) : (
          <IconButton
            icon={<X className="group-hover:text-[hsl(var(--accent-error))] transition-colors" />}
            tooltip="Close"
            onClick={onClose}
            size="sm"
            className="app-region-no-drag"
          />
        )}
      </header>
      <main className="flex flex-1 items-center justify-center p-6">
        <section
          aria-labelledby="launcher-root-recovery-title"
          className="w-full max-w-md rounded-lg border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-low))] p-6 shadow-lg"
        >
          <h1
            ref={headingRef}
            id="launcher-root-recovery-title"
            tabIndex={-1}
            className="text-lg font-semibold outline-none"
          >
            {title}
          </h1>
          <p
            role="status"
            aria-live="polite"
            aria-atomic="true"
            className="mt-2 text-sm text-[hsl(var(--text-secondary))]"
          >
            {message}
          </p>
          {(primaryAction || secondaryAction) && (
            <div className="mt-5 flex flex-wrap gap-3">
              {primaryAction && (
                <button
                  type="button"
                  onClick={primaryAction.onAction}
                  className="app-region-no-drag rounded bg-[hsl(var(--accent-primary))] px-4 py-2 text-sm font-medium text-black"
                >
                  {primaryAction.label}
                </button>
              )}
              {secondaryAction && (
                <button
                  type="button"
                  onClick={secondaryAction.onAction}
                  className="app-region-no-drag rounded border border-[hsl(var(--border-default))] px-4 py-2 text-sm"
                >
                  {secondaryAction.label}
                </button>
              )}
            </div>
          )}
        </section>
      </main>
    </div>
  );
}
