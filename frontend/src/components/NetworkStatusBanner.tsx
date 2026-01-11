/**
 * Network Status Banner Component
 *
 * Displays network status indicators for offline mode and rate limiting.
 * Shows when circuit breaker is open or when rate limits are approaching.
 */

import React from 'react';
import { WifiOff, AlertTriangle } from 'lucide-react';

interface NetworkStatusBannerProps {
  /** Whether any circuit breaker is open (offline mode) */
  isOffline: boolean;
  /** Whether rate limiting is approaching */
  isRateLimited: boolean;
  /** Current success rate as percentage */
  successRate: number;
  /** Number of circuit breaker rejections */
  circuitBreakerRejections: number;
}

export const NetworkStatusBanner: React.FC<NetworkStatusBannerProps> = ({
  isOffline,
  isRateLimited,
  successRate,
  circuitBreakerRejections,
}) => {
  // Don't render if everything is fine
  if (!isOffline && !isRateLimited) {
    return null;
  }

  // Offline banner takes priority
  if (isOffline) {
    return (
      <div className="bg-[hsl(var(--accent-warning)/0.15)] border-b border-[hsl(var(--accent-warning)/0.3)] px-4 py-2">
        <div className="flex items-center gap-2 text-[hsl(var(--accent-warning))]">
          <WifiOff className="w-4 h-4 flex-shrink-0" />
          <span className="text-sm font-medium">
            Using Cached Data
          </span>
          <span className="text-xs opacity-80">
            Network unavailable - showing cached results
          </span>
          {circuitBreakerRejections > 0 && (
            <span className="ml-auto text-xs opacity-60">
              {circuitBreakerRejections} requests blocked
            </span>
          )}
        </div>
      </div>
    );
  }

  // Rate limit warning
  if (isRateLimited) {
    return (
      <div className="bg-[hsl(var(--accent-warning)/0.15)] border-b border-[hsl(var(--accent-warning)/0.3)] px-4 py-2">
        <div className="flex items-center gap-2 text-[hsl(var(--accent-warning))]">
          <AlertTriangle className="w-4 h-4 flex-shrink-0" />
          <span className="text-sm font-medium">
            Rate Limit Warning
          </span>
          <span className="text-xs opacity-80">
            API rate limits approaching ({Math.round(successRate)}% success rate)
          </span>
        </div>
      </div>
    );
  }

  return null;
};
