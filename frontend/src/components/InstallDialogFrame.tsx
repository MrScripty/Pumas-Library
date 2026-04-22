import { useEffect, useId, useRef, type ReactNode } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { X } from 'lucide-react';

interface InstallDialogFrameProps {
  children: ReactNode;
  isOpen: boolean;
  isPageMode: boolean;
  onClose: () => void;
  title: string;
}

export function InstallDialogFrame({
  children,
  isOpen,
  isPageMode,
  onClose,
  title,
}: InstallDialogFrameProps) {
  const titleId = useId();
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isOpen || isPageMode) {
      return undefined;
    }

    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const focusTimer = window.setTimeout(() => closeButtonRef.current?.focus(), 0);

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleEscape);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener('keydown', handleEscape);
      previousFocus?.focus();
    };
  }, [isOpen, isPageMode, onClose]);

  if (!isOpen) {
    return null;
  }

  const containerClasses = isPageMode
    ? 'w-full h-full flex flex-col'
    : 'w-full max-w-3xl max-h-[80vh] flex flex-col';

  const dialogContent = (
    <div
      role={isPageMode ? undefined : 'dialog'}
      aria-modal={isPageMode ? undefined : true}
      aria-labelledby={isPageMode ? undefined : titleId}
      className={containerClasses}
    >
      {!isPageMode && (
        <div className="flex items-center justify-between p-4 border-b border-[hsl(var(--border-default))]">
          <div className="flex items-center gap-3">
            <h2 id={titleId} className="text-xl font-semibold text-[hsl(var(--text-primary))]">
              {title}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              ref={closeButtonRef}
              onClick={onClose}
              className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors"
              aria-label="Close install dialog"
            >
              <X size={20} className="text-[hsl(var(--text-muted))]" />
            </button>
          </div>
        </div>
      )}

      {children}
    </div>
  );

  if (isPageMode) {
    return (
      <div className="w-full h-full flex flex-col">
        {dialogContent}
      </div>
    );
  }

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.button
            type="button"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="fixed inset-0 bg-black/70 z-50"
            aria-label="Dismiss install dialog"
          />
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none"
          >
            <div className="pointer-events-auto">
              {dialogContent}
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
