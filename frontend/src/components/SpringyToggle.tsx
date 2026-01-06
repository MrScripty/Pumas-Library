import React from 'react';
import { motion } from 'framer-motion';

interface SpringyToggleProps {
  isOn: boolean;
  onToggle: () => void;
  labelOn: string;
  labelOff: string;
  disabled?: boolean;
}

export const SpringyToggle: React.FC<SpringyToggleProps> = ({
  isOn,
  onToggle,
  labelOn,
  labelOff,
  disabled = false
}) => {
  return (
    <div
      className={`relative w-[220px] h-[36px] bg-[hsl(var(--surface-interactive))] border border-[hsl(var(--border-control))] overflow-hidden cursor-pointer select-none ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
      onClick={!disabled ? onToggle : undefined}
      onKeyDown={!disabled ? (e: React.KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onToggle(); } } : undefined}
      role="button"
      tabIndex={disabled ? -1 : 0}
      aria-pressed={isOn}
      aria-disabled={disabled}
    >
      {/* Background Labels */}
      <div className="absolute inset-0 flex items-center justify-between px-4 z-0">
        <span className={`text-[9px] font-bold w-1/2 text-center transition-colors duration-300 ${!isOn ? 'text-[hsl(var(--text-secondary))]' : 'text-[hsl(var(--text-tertiary))]'}`}>
          {labelOff}
        </span>
        <span className={`text-[9px] font-bold w-1/2 text-center transition-colors duration-300 ${isOn ? 'text-[hsl(var(--text-primary))]' : 'text-[hsl(var(--text-tertiary))]'}`}>
          {labelOn}
        </span>
      </div>

      {/* Sliding Tab */}
      <motion.div
        className="absolute top-[2px] bottom-[2px] w-[108px] z-10"
        initial={false}
        animate={{
          x: isOn ? 108 : 2, // 2px padding
          backgroundColor: isOn
            ? 'hsl(var(--accent-success))'
            : 'hsl(var(--surface-interactive-hover))',
        }}
        transition={{
          type: "spring",
          stiffness: 500,
          damping: 30
        }}
      />
    </div>
  );
};
