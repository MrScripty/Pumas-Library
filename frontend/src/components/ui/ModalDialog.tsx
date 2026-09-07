import {
  useEffect,
  useId,
  useRef,
  type MouseEvent,
  type ReactNode,
  type RefObject,
} from 'react';
import { createPortal } from 'react-dom';
import { AnimatePresence, motion } from 'framer-motion';
import { registerOverlayEscapeLayer } from './OverlayEscapeStack';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

type ModalEntry = {
  id: string;
  dialogRef: RefObject<HTMLDivElement | null>;
  initialFocusRef?: RefObject<HTMLElement | null>;
  restoreFocus: HTMLElement | null;
  restoreFocusFallbackRef?: RefObject<HTMLElement | null>;
};

const modalStack: ModalEntry[] = [];

function getTopModal(): ModalEntry | undefined {
  for (let index = modalStack.length - 1; index >= 0; index -= 1) {
    const entry = modalStack[index];
    if (entry?.dialogRef.current?.isConnected) {
      return entry;
    }
  }
  return undefined;
}

function getFocusableElements(dialog: HTMLElement): HTMLElement[] {
  return Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.hidden && element.getAttribute('aria-hidden') !== 'true'
  );
}

function focusModal(entry: ModalEntry): void {
  const dialog = entry.dialogRef.current;
  if (!dialog?.isConnected) {
    return;
  }

  const requestedTarget = entry.initialFocusRef?.current;
  if (requestedTarget?.isConnected && dialog.contains(requestedTarget)) {
    requestedTarget.focus();
    return;
  }

  const firstFocusable = getFocusableElements(dialog)[0];
  (firstFocusable ?? dialog).focus();
}

function restoreModalFocus(entry: ModalEntry): void {
  if (entry.restoreFocus?.isConnected) {
    entry.restoreFocus.focus();
    return;
  }

  const parentModal = getTopModal();
  if (parentModal) {
    focusModal(parentModal);
    return;
  }
  const fallback = entry.restoreFocusFallbackRef?.current;
  if (fallback?.isConnected) fallback.focus();
}

export interface ModalDialogProps {
  ariaDescribedBy?: string;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  backdropClassName?: string;
  children: ReactNode;
  contentClassName?: string;
  dismissDisabled?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  /** Owning task's destination when its original opener was removed. */
  restoreFocusFallbackRef?: RefObject<HTMLElement | null>;
  isOpen: boolean;
  onClose: () => void;
  overlayClassName?: string;
  role?: 'alertdialog' | 'dialog';
  shouldCloseOnBackdrop?: boolean;
}

/**
 * Owns modal semantics and lifecycle while feature consumers own content and
 * domain actions. Nested instances share one topmost-focus stack.
 */
export function ModalDialog({
  ariaDescribedBy,
  ariaLabel,
  ariaLabelledBy,
  backdropClassName = 'bg-black/60 backdrop-blur-sm',
  children,
  contentClassName = '',
  dismissDisabled = false,
  initialFocusRef,
  restoreFocusFallbackRef,
  isOpen,
  onClose,
  overlayClassName = 'fixed inset-0 z-50 flex items-center justify-center p-4',
  role = 'dialog',
  shouldCloseOnBackdrop = true,
}: ModalDialogProps) {
  const generatedId = useId();
  const entryId = useRef(`modal-${generatedId}`);
  const dialogRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  const dismissDisabledRef = useRef(dismissDisabled);
  onCloseRef.current = onClose;
  dismissDisabledRef.current = dismissDisabled;

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const entry: ModalEntry = {
      id: entryId.current,
      dialogRef,
      initialFocusRef,
      restoreFocusFallbackRef,
      restoreFocus: document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null,
    };
    // React can clear the ref before passive cleanup. Retain the owned node
    // so a closing parent can still transfer its opener to nested dialogs.
    const ownedDialog = dialogRef.current;
    modalStack.push(entry);
    const unregisterEscapeLayer = registerOverlayEscapeLayer(entry.id, () => {
      if (!dismissDisabledRef.current) {
        onCloseRef.current();
      }
    });

    const focusTimer = window.setTimeout(() => {
      if (getTopModal()?.id === entry.id) {
        focusModal(entry);
      }
    }, 0);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (getTopModal()?.id !== entry.id) {
        return;
      }

      if (event.key !== 'Tab') {
        return;
      }

      const dialog = dialogRef.current;
      if (!dialog) {
        return;
      }

      const focusableElements = getFocusableElements(dialog);
      const firstElement = focusableElements[0];
      const lastElement = focusableElements.at(-1);
      if (!firstElement || !lastElement) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const activeElement = document.activeElement;
      if (event.shiftKey && (activeElement === firstElement || !dialog.contains(activeElement))) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && (activeElement === lastElement || !dialog.contains(activeElement))) {
        event.preventDefault();
        firstElement.focus();
      }
    };

    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current;
      if (
        getTopModal()?.id === entry.id &&
        dialog &&
        event.target instanceof Node &&
        !dialog.contains(event.target)
      ) {
        focusModal(entry);
      }
    };

    document.addEventListener('keydown', handleKeyDown, true);
    document.addEventListener('focusin', handleFocusIn, true);
    return () => {
      window.clearTimeout(focusTimer);
      document.removeEventListener('keydown', handleKeyDown, true);
      document.removeEventListener('focusin', handleFocusIn, true);
      unregisterEscapeLayer();

      const entryIndex = modalStack.findIndex((candidate) => candidate.id === entry.id);
      const wasTopModal = entryIndex === modalStack.length - 1;
      if (entryIndex >= 0) {
        modalStack.splice(entryIndex, 1);
      }
      const removedDialog = ownedDialog;
      if (!wasTopModal && removedDialog) {
        for (const descendant of modalStack) {
          if (descendant.restoreFocus && removedDialog.contains(descendant.restoreFocus)) {
            descendant.restoreFocus = entry.restoreFocus;
          }
        }
      }
      if (wasTopModal) {
        restoreModalFocus(entry);
      }
    };
  }, [initialFocusRef, isOpen, restoreFocusFallbackRef]);

  const handleBackdropMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (shouldCloseOnBackdrop && !dismissDisabled) {
      onClose();
    }
  };

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className={overlayClassName}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
        >
          <div
            aria-hidden="true"
            className={`absolute inset-0 ${backdropClassName}`}
            data-modal-backdrop=""
            onMouseDown={handleBackdropMouseDown}
          />
          <motion.div
            ref={dialogRef}
            role={role}
            aria-modal="true"
            aria-label={ariaLabel}
            aria-labelledby={ariaLabelledBy}
            aria-describedby={ariaDescribedBy}
            className={`relative ${contentClassName}`}
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            tabIndex={-1}
          >
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body
  );
}
