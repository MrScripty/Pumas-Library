import { useEffect, useId, useRef } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { AlertTriangle, X } from 'lucide-react';

interface ConfirmationDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  isConfirming?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ConfirmationDialog({
  isOpen,
  title,
  message,
  confirmLabel,
  cancelLabel = 'Cancel',
  isConfirming = false,
  onCancel,
  onConfirm,
}: ConfirmationDialogProps) {
  const titleId = useId();
  const messageId = useId();
  const cancelButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const focusTimer = window.setTimeout(() => cancelButtonRef.current?.focus(), 0);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        event.stopImmediatePropagation();
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener('keydown', handleKeyDown, true);
      previousFocus?.focus();
    };
  }, [isOpen, onCancel]);

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-[70] flex items-center justify-center p-4 pointer-events-none"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
        >
          <motion.button
            type="button"
            className="absolute inset-0 bg-black/60 backdrop-blur-sm pointer-events-auto"
            aria-label="Dismiss confirmation"
            onClick={onCancel}
            disabled={isConfirming}
          />

          <motion.div
            role="alertdialog"
            aria-modal="true"
            aria-labelledby={titleId}
            aria-describedby={messageId}
            className="relative pointer-events-auto w-full max-w-md rounded-lg border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-primary))] shadow-xl"
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.18 }}
          >
            <div className="flex items-start justify-between gap-4 border-b border-[hsl(var(--launcher-border)/0.5)] px-5 py-4">
              <div className="flex items-start gap-3">
                <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-[hsl(var(--accent-warning))]" />
                <div>
                  <h2 id={titleId} className="text-sm font-semibold text-[hsl(var(--launcher-text-primary))]">
                    {title}
                  </h2>
                  <p id={messageId} className="mt-1 text-sm text-[hsl(var(--launcher-text-secondary))]">
                    {message}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={onCancel}
                disabled={isConfirming}
                className="rounded-md p-1 text-[hsl(var(--launcher-text-muted))] transition-colors hover:bg-[hsl(var(--launcher-bg-secondary))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
                aria-label="Close confirmation"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className="flex justify-end gap-2 px-5 py-4">
              <button
                type="button"
                ref={cancelButtonRef}
                onClick={onCancel}
                disabled={isConfirming}
                className="rounded-md border border-[hsl(var(--launcher-border))] px-3 py-2 text-sm text-[hsl(var(--launcher-text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-secondary))] hover:text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
              >
                {cancelLabel}
              </button>
              <button
                type="button"
                onClick={onConfirm}
                disabled={isConfirming}
                className="rounded-md bg-[hsl(var(--accent-warning))] px-3 py-2 text-sm font-medium text-[hsl(var(--launcher-bg-primary))] transition-colors hover:bg-[hsl(var(--accent-warning)/0.85)] disabled:opacity-50"
              >
                {confirmLabel}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
