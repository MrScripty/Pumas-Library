/**
 * Status Display Component
 *
 * Shows system status message.
 * Extracted from App.tsx for better organization.
 */


interface StatusDisplayProps {
  message: string;
  isRunning: boolean;
  isSetupComplete: boolean;
}

export function StatusDisplay({
  message,
  isRunning,
  isSetupComplete,
}: StatusDisplayProps) {
  if (!message) {
    return null;
  }

  return (
    <div className="h-6 text-center w-full px-2">
      <span
        className={`text-sm italic font-medium transition-colors duration-300 block truncate ${
          isRunning
            ? 'text-[hsl(var(--accent-success))]'
            : isSetupComplete
            ? 'text-[hsl(var(--accent-success))]'
            : 'text-[hsl(var(--text-tertiary))]'
        }`}
      >
        {message}
      </span>
    </div>
  );
}
