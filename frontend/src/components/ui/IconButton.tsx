import React, { useState } from 'react';
import { useHover } from '@react-aria/interactions';

type IconButtonSize = 'sm' | 'md' | 'lg';
type IconButtonVariant = 'ghost' | 'subtle' | 'solid';
type TooltipPosition = 'top' | 'bottom' | 'left' | 'right';

interface IconButtonProps {
  icon: React.ReactNode;
  tooltip: string;
  onClick?: () => void;
  size?: IconButtonSize;
  variant?: IconButtonVariant;
  tooltipPosition?: TooltipPosition;
  disabled?: boolean;
  className?: string;
  active?: boolean;
}

const sizeClasses: Record<IconButtonSize, string> = {
  sm: 'p-1',
  md: 'p-1.5',
  lg: 'p-2',
};

const iconSizeClasses: Record<IconButtonSize, string> = {
  sm: '[&>svg]:w-3.5 [&>svg]:h-3.5',
  md: '[&>svg]:w-4 [&>svg]:h-4',
  lg: '[&>svg]:w-5 [&>svg]:h-5',
};

const variantClasses: Record<IconButtonVariant, string> = {
  ghost: 'hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]',
  subtle: 'bg-[hsl(var(--surface-low))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))]',
  solid: 'bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary)/0.8)] text-[hsl(0_0%_10%)]',
};

const tooltipPositionClasses: Record<TooltipPosition, string> = {
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-1',
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-1',
  left: 'right-full top-1/2 -translate-y-1/2 mr-1',
  right: 'left-full top-1/2 -translate-y-1/2 ml-1',
};

export const IconButton: React.FC<IconButtonProps> = ({
  icon,
  tooltip,
  onClick,
  size = 'md',
  variant = 'ghost',
  tooltipPosition = 'top',
  disabled = false,
  className = '',
  active = false,
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const { hoverProps } = useHover({
    onHoverStart: () => setIsHovered(true),
    onHoverEnd: () => setIsHovered(false),
    isDisabled: disabled,
  });

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-label={tooltip}
      className={`
        relative inline-flex items-center justify-center rounded transition-colors
        ${sizeClasses[size]}
        ${iconSizeClasses[size]}
        ${variantClasses[variant]}
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        ${active ? 'bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-primary))]' : ''}
        ${className}
      `.trim().replace(/\s+/g, ' ')}
      {...hoverProps}
    >
      {icon}
      {isHovered && tooltip && !disabled && (
        <div
          className={`absolute ${tooltipPositionClasses[tooltipPosition]} px-1.5 py-0.5 bg-[hsl(var(--surface-overlay))] border border-[hsl(var(--border-default))] rounded text-[10px] text-[hsl(var(--text-primary))] whitespace-nowrap z-50 pointer-events-none`}
          role="tooltip"
        >
          {tooltip}
        </div>
      )}
    </button>
  );
};
