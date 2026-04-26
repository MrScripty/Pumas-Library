import type { CSSProperties } from 'react';

interface LocalModelDownloadProgressRingProps {
  isPaused?: boolean;
  isQueued?: boolean;
  isRetained?: boolean;
  ringDegrees: number;
}

export function LocalModelDownloadProgressRing({
  isPaused = false,
  isQueued = false,
  isRetained = false,
  ringDegrees,
}: LocalModelDownloadProgressRingProps) {
  return (
    <>
      <span
        className={[
          'download-progress-ring',
          isQueued ? 'is-waiting' : '',
          isPaused ? 'is-paused' : '',
          isRetained ? 'is-retained' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        style={{ '--progress': `${ringDegrees}deg` } as CSSProperties}
      />
      {!isQueued && !isPaused && !isRetained && <span className="download-scan-ring" />}
    </>
  );
}
