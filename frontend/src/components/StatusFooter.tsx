import React from 'react';
import { WifiOff, Wifi, RefreshCw, Clock, Database } from 'lucide-react';

interface StatusFooterProps {
  cacheStatus: {
    has_cache: boolean;
    is_valid: boolean;
    is_fetching: boolean;
    age_seconds?: number;
    last_fetched?: string;
    releases_count?: number;
  };
}

export const StatusFooter: React.FC<StatusFooterProps> = ({ cacheStatus }) => {
  const getStatusInfo = () => {
    // FETCHING STATE
    if (cacheStatus.is_fetching) {
      return {
        icon: RefreshCw,
        text: 'Fetching releases...',
        color: 'text-blue-400',
        bgColor: 'bg-blue-500/10',
        spinning: true
      };
    }

    // NO CACHE STATE
    if (!cacheStatus.has_cache) {
      return {
        icon: WifiOff,
        text: 'No cache available - offline mode',
        color: 'text-orange-400',
        bgColor: 'bg-orange-500/10',
        spinning: false
      };
    }

    // VALID CACHE STATE
    if (cacheStatus.is_valid) {
      const ageMinutes = cacheStatus.age_seconds
        ? Math.floor(cacheStatus.age_seconds / 60)
        : 0;

      return {
        icon: Database,
        text: `Cached data (${ageMinutes}m old) · ${cacheStatus.releases_count || 0} releases`,
        color: 'text-green-400',
        bgColor: 'bg-green-500/10',
        spinning: false
      };
    }

    // STALE CACHE STATE
    const ageHours = cacheStatus.age_seconds
      ? Math.floor(cacheStatus.age_seconds / 3600)
      : 0;

    return {
      icon: Clock,
      text: `Stale cache (${ageHours}h old) · offline mode`,
      color: 'text-yellow-400',
      bgColor: 'bg-yellow-500/10',
      spinning: false
    };
  };

  const status = getStatusInfo();
  const Icon = status.icon;

  return (
    <div className={`
      fixed bottom-0 left-0 right-0
      ${status.bgColor} border-t border-gray-700/50
      px-4 py-2 flex items-center gap-2
      text-xs font-medium ${status.color}
      z-50
    `}>
      <Icon
        className={`w-3.5 h-3.5 ${status.spinning ? 'animate-spin' : ''}`}
      />
      <span>{status.text}</span>
    </div>
  );
};
