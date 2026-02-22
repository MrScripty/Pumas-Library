/**
 * Hold-to-Delete Button
 *
 * A destructive action button that requires a 2-second press-and-hold
 * to confirm deletion. Shows a circular progress ring during the hold.
 */

import React, { useState, useRef, useCallback, type CSSProperties } from 'react';
import { Trash2, Loader2 } from 'lucide-react';
import { useHover } from '@react-aria/interactions';

const HOLD_DURATION_MS = 2000;

interface HoldToDeleteButtonProps {
  onDelete: () => void | Promise<void>;
  disabled?: boolean;
  tooltip?: string;
}

export const HoldToDeleteButton: React.FC<HoldToDeleteButtonProps> = ({
  onDelete,
  disabled = false,
  tooltip = 'Hold to delete',
}) => {
  const [isHolding, setIsHolding] = useState(false);
  const [holdProgress, setHoldProgress] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rafRef = useRef<number | null>(null);
  const startTimeRef = useRef<number>(0);
  const [isHovered, setIsHovered] = useState(false);
  const { hoverProps } = useHover({
    onHoverStart: () => setIsHovered(true),
    onHoverEnd: () => setIsHovered(false),
    isDisabled: disabled || isDeleting,
  });

  const cancelHold = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (rafRef.current) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    setIsHolding(false);
    setHoldProgress(0);
  }, []);

  const animateProgress = useCallback(() => {
    const elapsed = performance.now() - startTimeRef.current;
    const progress = Math.min(1, elapsed / HOLD_DURATION_MS);
    setHoldProgress(progress);
    if (progress < 1) {
      rafRef.current = requestAnimationFrame(animateProgress);
    }
  }, []);

  const startHold = useCallback(() => {
    if (disabled || isDeleting) return;
    setIsHolding(true);
    setHoldProgress(0);
    startTimeRef.current = performance.now();
    rafRef.current = requestAnimationFrame(animateProgress);
    timerRef.current = setTimeout(async () => {
      timerRef.current = null;
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      setIsHolding(false);
      setHoldProgress(0);
      setIsDeleting(true);
      try {
        await onDelete();
      } finally {
        setIsDeleting(false);
      }
    }, HOLD_DURATION_MS);
  }, [disabled, isDeleting, onDelete, animateProgress]);

  const progressDegrees = Math.round(holdProgress * 360);
  const currentTooltip = isDeleting ? 'Deleting...' : tooltip;

  return (
    <button
      onPointerDown={startHold}
      onPointerUp={cancelHold}
      onPointerLeave={cancelHold}
      onPointerCancel={cancelHold}
      disabled={disabled || isDeleting}
      aria-label={currentTooltip}
      className={`
        relative inline-flex items-center justify-center rounded p-1 transition-colors
        [&>svg]:w-3.5 [&>svg]:h-3.5
        ${disabled || isDeleting
          ? 'opacity-50 cursor-not-allowed'
          : 'cursor-pointer hover:bg-[hsl(var(--surface-interactive-hover))]'}
        ${isHolding
          ? 'text-[hsl(var(--launcher-accent-error))]'
          : 'text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]'}
      `.trim().replace(/\s+/g, ' ')}
      {...hoverProps}
    >
      {isHolding && (
        <span
          className="delete-hold-ring"
          style={{ '--progress': `${progressDegrees}deg` } as CSSProperties}
        />
      )}
      {isDeleting ? (
        <Loader2 className="animate-spin" />
      ) : (
        <Trash2 />
      )}
      {isHovered && currentTooltip && !disabled && (
        <div
          className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-1.5 py-0.5 bg-[hsl(var(--surface-overlay))] border border-[hsl(var(--border-default))] rounded text-[10px] text-[hsl(var(--text-primary))] whitespace-nowrap z-50 pointer-events-none"
          role="tooltip"
        >
          {currentTooltip}
        </div>
      )}
    </button>
  );
};
