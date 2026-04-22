import { Tag } from 'lucide-react';

interface LocalModelGroupHeaderProps {
  category: string;
  modelCount: number;
}

export function LocalModelGroupHeader({
  category,
  modelCount,
}: LocalModelGroupHeaderProps) {
  return (
    <div className="flex items-center gap-2 px-1">
      <Tag className="w-3.5 h-3.5 text-[hsl(var(--text-muted))]" />
      <p className="text-xs font-semibold text-[hsl(var(--text-muted))] uppercase tracking-wider">
        {category}
      </p>
      <span className="text-xs text-[hsl(var(--text-muted))]">
        ({modelCount})
      </span>
    </div>
  );
}
