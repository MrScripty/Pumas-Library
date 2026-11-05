import React, { useId, useState } from 'react';
import { useFocusWithin, useHover, useKeyboard } from '@react-aria/interactions';

type TooltipPosition = 'top' | 'bottom' | 'left' | 'right';

interface TooltipProps {
  children: React.ReactNode;
  content: string;
  position?: TooltipPosition;
  className?: string;
}

const positionClasses: Record<TooltipPosition, string> = {
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-1',
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-1',
  left: 'right-full top-1/2 -translate-y-1/2 mr-1',
  right: 'left-full top-1/2 -translate-y-1/2 ml-1',
};

type DescribableProps = { 'aria-describedby'?: string };

/**
 * Describes its trigger. Opens on pointer hover and on keyboard focus, closes
 * on Escape, and stays in the DOM while hidden so `aria-describedby` on the
 * trigger always resolves.
 */
export const Tooltip: React.FC<TooltipProps> = ({
  children,
  content,
  position = 'top',
  className = '',
}) => {
  const tooltipId = useId();
  const [isHovered, setIsHovered] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const [isDismissed, setIsDismissed] = useState(false);
  const { hoverProps } = useHover({
    onHoverStart: () => {
      setIsDismissed(false);
      setIsHovered(true);
    },
    onHoverEnd: () => setIsHovered(false),
  });
  const { focusWithinProps } = useFocusWithin({
    onFocusWithinChange: (focused) => {
      if (focused) setIsDismissed(false);
      setIsFocused(focused);
    },
  });
  const hasContent = content.length > 0;
  const isOpen = hasContent && !isDismissed && (isHovered || isFocused);
  const { keyboardProps } = useKeyboard({
    onKeyDown: (event) => {
      if (event.key === 'Escape' && isOpen) {
        setIsDismissed(true);
      }
      // Escape and every other key still reach enclosing dialogs and lists.
      event.continuePropagation();
    },
  });

  // The single interactive child is the trigger; otherwise the wrapper is.
  const childElement = React.Children.count(children) === 1 && React.isValidElement<DescribableProps>(children)
    ? children
    : null;
  const describedBy = (existing?: string) => [existing, tooltipId].filter(Boolean).join(' ');
  const trigger = childElement && hasContent
    ? React.cloneElement(childElement, {
      'aria-describedby': describedBy(childElement.props['aria-describedby']),
    })
    : children;

  return (
    <div
      className={`relative inline-flex ${className}`}
      {...hoverProps}
      {...focusWithinProps}
      {...keyboardProps}
      aria-describedby={!childElement && hasContent ? tooltipId : undefined}
    >
      {trigger}
      {hasContent && (
        <div
          id={tooltipId}
          className={`absolute ${positionClasses[position]} px-1.5 py-0.5 bg-[hsl(var(--surface-overlay))] border border-[hsl(var(--border-default))] rounded text-[10px] text-[hsl(var(--text-primary))] whitespace-nowrap z-50 pointer-events-none`}
          role="tooltip"
          hidden={!isOpen}
        >
          {content}
        </div>
      )}
    </div>
  );
};
