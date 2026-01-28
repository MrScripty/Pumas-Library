import React from 'react';

interface EmptyStateProps {
  icon: React.ReactNode;
  message: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  icon,
  message,
  action,
  className = '',
}) => {
  return (
    <div className={`flex flex-col items-center justify-center h-64 text-[hsl(var(--text-muted))] ${className}`}>
      <div className="w-10 h-10 mb-3 opacity-50 [&>svg]:w-full [&>svg]:h-full">
        {icon}
      </div>
      <p className="text-sm text-center max-w-[240px]">{message}</p>
      {action && (
        <button
          onClick={action.onClick}
          className="mt-2 text-xs text-[hsl(var(--accent-primary))] hover:underline"
        >
          {action.label}
        </button>
      )}
    </div>
  );
};
