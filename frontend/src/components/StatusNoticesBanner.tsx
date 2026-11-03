/**
 * Explains degraded subsystems reported by the backend status (for example,
 * downloads disabled because saved download state could not be read). The
 * rest of the app remains usable while these are shown.
 */
import { AlertTriangle } from 'lucide-react';

interface StatusNoticesBannerProps {
  notices: readonly string[];
}

export function StatusNoticesBanner({ notices }: StatusNoticesBannerProps) {
  if (notices.length === 0) return null;
  return (
    <div
      role="status"
      className="relative z-10 bg-[hsl(var(--accent-warning)/0.15)] border-b border-[hsl(var(--accent-warning)/0.3)] px-4 py-2"
    >
      <ul className="space-y-1">
        {notices.map((notice) => (
          <li key={notice} className="flex items-start gap-2 text-[hsl(var(--accent-warning))]">
            <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" aria-hidden="true" />
            <span className="text-xs break-words">{notice}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
