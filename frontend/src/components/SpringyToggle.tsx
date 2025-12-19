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
      className={`relative w-[220px] h-[36px] bg-[#2d2d2d] border border-[#3d3d3d] overflow-hidden cursor-pointer select-none ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
      onClick={!disabled ? onToggle : undefined}
    >
      {/* Background Labels */}
      <div className="absolute inset-0 flex items-center justify-between px-4 z-0">
        <span className={`text-[9px] font-bold w-1/2 text-center transition-colors duration-300 ${!isOn ? 'text-[#aaaaaa]' : 'text-[#666666]'}`}>
          {labelOff}
        </span>
        <span className={`text-[9px] font-bold w-1/2 text-center transition-colors duration-300 ${isOn ? 'text-white' : 'text-[#888888]'}`}>
          {labelOn}
        </span>
      </div>

      {/* Sliding Tab */}
      <motion.div
        className="absolute top-[2px] bottom-[2px] w-[108px] z-10"
        initial={false}
        animate={{
          x: isOn ? 108 : 2, // 2px padding
          backgroundColor: isOn ? '#55ff55' : '#444444',
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