import { ChevronDown, ChevronUp } from 'lucide-react';
import { AnimatePresence, motion } from 'framer-motion';
import type { ReactNode } from 'react';

interface MappingPreviewListSectionProps {
  accentBorderClass?: string;
  buttonHoverClass?: string;
  children: ReactNode;
  count: number;
  expandedSection: string | null;
  icon: ReactNode;
  sectionId: string;
  title: string;
  titleClassName?: string;
  onToggle: (section: string) => void;
}

export function MappingPreviewListSection({
  accentBorderClass = 'border-[hsl(var(--launcher-border)/0.3)]',
  buttonHoverClass = 'hover:bg-[hsl(var(--launcher-bg-secondary)/0.3)]',
  children,
  count,
  expandedSection,
  icon,
  sectionId,
  title,
  titleClassName = 'text-[hsl(var(--launcher-text-primary))]',
  onToggle,
}: MappingPreviewListSectionProps) {
  return (
    <div className={`border rounded ${accentBorderClass}`}>
      <button
        onClick={() => onToggle(sectionId)}
        className={`w-full px-3 py-2 flex items-center justify-between text-left transition-colors ${buttonHoverClass}`}
      >
        <div className="flex items-center gap-2">
          {icon}
          <span className={`text-xs font-medium ${titleClassName}`}>
            {title} ({count})
          </span>
        </div>
        {expandedSection === sectionId ? (
          <ChevronUp className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
        ) : (
          <ChevronDown className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
        )}
      </button>
      <AnimatePresence>
        {expandedSection === sectionId && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden"
          >
            {children}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
