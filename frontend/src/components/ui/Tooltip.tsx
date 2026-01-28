import React, { useState } from 'react';
import { useHover } from '@react-aria/interactions';

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

export const Tooltip: React.FC<TooltipProps> = ({
  children,
  content,
  position = 'top',
  className = '',
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const { hoverProps } = useHover({
    onHoverStart: () => setIsHovered(true),
    onHoverEnd: () => setIsHovered(false),
  });

  return (
    <div className={`relative inline-flex ${className}`} {...hoverProps}>
      {children}
      {isHovered && content && (
        <div
          className={`absolute ${positionClasses[position]} px-1.5 py-0.5 bg-[hsl(var(--surface-overlay))] border border-[hsl(var(--border-default))] rounded text-[10px] text-[hsl(var(--text-primary))] whitespace-nowrap z-50 pointer-events-none`}
          role="tooltip"
        >
          {content}
        </div>
      )}
    </div>
  );
};
