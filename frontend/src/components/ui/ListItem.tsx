import React from 'react';

interface ListItemProps {
  children: React.ReactNode;
  highlighted?: boolean;
  className?: string;
}

export const ListItem: React.FC<ListItemProps> = ({
  children,
  highlighted = false,
  className = '',
}) => {
  return (
    <div
      className={`
        rounded transition-colors group
        ${highlighted
          ? 'bg-[hsl(var(--surface-low)/0.4)] hover:bg-[hsl(var(--surface-low)/0.6)]'
          : 'bg-[hsl(var(--surface-low)/0.2)] hover:bg-[hsl(var(--surface-low)/0.35)]'
        }
        ${className}
      `.trim().replace(/\s+/g, ' ')}
    >
      {children}
    </div>
  );
};

interface ListItemContentProps {
  children: React.ReactNode;
  className?: string;
}

export const ListItemContent: React.FC<ListItemContentProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`flex items-center justify-between p-2 gap-2 ${className}`}>
      {children}
    </div>
  );
};

interface MetadataRowProps {
  children: React.ReactNode;
  className?: string;
}

export const MetadataRow: React.FC<MetadataRowProps> = ({
  children,
  className = '',
}) => {
  return (
    <div className={`flex items-center gap-2 mt-1 text-xs text-[hsl(var(--text-muted))] ${className}`}>
      {children}
    </div>
  );
};

interface MetadataItemProps {
  icon: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

export const MetadataItem: React.FC<MetadataItemProps> = ({
  icon,
  children,
  className = '',
}) => {
  return (
    <span className={`inline-flex items-center gap-1 [&>svg]:w-3 [&>svg]:h-3 ${className}`}>
      {icon}
      {children}
    </span>
  );
};
