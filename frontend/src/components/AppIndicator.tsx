import React, { useState, useEffect } from 'react';
import { Play, Square, AlertTriangle, FileText } from 'lucide-react';
import { useHover } from '@react-aria/interactions';
import { getLogger } from '../utils/logger';

const logger = getLogger('AppIndicator');

interface AppIndicatorProps {
  appId: string;
  state: 'running' | 'offline' | 'uninstalled' | 'error';
  isSelected: boolean;
  hasInstall: boolean;
  launchError: boolean;
  onLaunch?: () => void;
  onStop?: () => void;
  onOpenLog?: () => void;
}

export const AppIndicator: React.FC<AppIndicatorProps> = ({
  appId,
  state,
  hasInstall,
  launchError,
  onLaunch,
  onStop,
  onOpenLog,
}) => {
  const [spinnerFrame, setSpinnerFrame] = useState(0);
  const [errorFlash, setErrorFlash] = useState(false);

  // Use React Aria's battle-tested hover hook
  const { hoverProps, isHovered } = useHover({});
  const isHovering = isHovered;

  const spinnerFrames = ['/', '-', '\\', '|'];
  const iconSize = 60;
  const indicatorRadius = iconSize / 3; // Larger indicator (1/3 icon radius)

  // Spinner animation for running state
  useEffect(() => {
    if (state === 'running') {
      logger.debug(`Starting spinner animation for app: ${appId}`);
      const interval = setInterval(() => {
        setSpinnerFrame(prev => (prev + 1) % spinnerFrames.length);
      }, 150);
      return () => {
        logger.debug(`Stopping spinner animation for app: ${appId}`);
        clearInterval(interval);
      };
    }
    return undefined;
  }, [state, appId]);

  // Error flash animation
  useEffect(() => {
    if (launchError) {
      logger.warn(`Launch error detected for app: ${appId}, starting error flash animation`);
      const interval = setInterval(() => {
        setErrorFlash(prev => !prev);
      }, 500);
      return () => {
        logger.debug(`Stopping error flash animation for app: ${appId}`);
        clearInterval(interval);
      };
    } else {
      setErrorFlash(false);
    }
    return undefined;
  }, [launchError, appId]);


  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();

    if (state === 'running') {
      logger.info(`Stop requested for app: ${appId}`);
      onStop?.();
    } else if (state === 'error' && isHovering) {
      logger.info(`Open log requested for app: ${appId}`);
      onOpenLog?.();
    } else if ((state === 'offline' || state === 'error') && hasInstall) {
      logger.info(`Launch requested for app: ${appId}, state: ${state}`);
      onLaunch?.();
    } else {
      logger.debug(`Click ignored for app: ${appId}, state: ${state}, hasInstall: ${hasInstall}, isHovering: ${isHovering}`);
    }
  };

  const getIndicatorContent = () => {
    // Running state
    if (state === 'running') {
      if (isHovering) {
        // Red stop square on hover
        return (
          <Square
            className="w-3.5 h-3.5 text-[hsl(var(--accent-error))]"
            fill="currentColor"
            stroke="currentColor"
            strokeWidth={1}
          />
        );
      } else {
        // Spinning line inside a circle
        return (
          <div className="relative w-full h-full flex items-center justify-center">
            <div
              className="absolute w-full h-full bg-[hsl(var(--launcher-accent-success))] rounded-full opacity-20"
              style={{
                boxShadow: `
                  0 0 4px 0px hsl(var(--launcher-accent-success) / 0.6),
                  0 0 8px 2px hsl(var(--launcher-accent-success) / 0.4),
                  0 0 12px 4px hsl(var(--launcher-accent-success) / 0.2),
                  0 0 16px 6px hsl(var(--launcher-accent-success) / 0.1),
                  0 0 20px 8px hsl(var(--launcher-accent-success) / 0.05)
                `
              }}
            />
            <span className="font-mono text-[13px] font-bold text-[hsl(var(--launcher-accent-success))] relative z-10">
              {spinnerFrames[spinnerFrame]}
            </span>
          </div>
        );
      }
    }

    // Error state
    if (state === 'error' || launchError) {
      if (isHovering) {
        // Log icon on hover
        return (
          <FileText className="w-3.5 h-3.5 text-[hsl(var(--accent-error))]" />
        );
      } else {
        // Alternate between triangle and play
        return errorFlash ? (
          <AlertTriangle className="w-3.5 h-3.5 text-[hsl(var(--accent-error))]" />
        ) : (
          <Play className="w-3.5 h-3.5 text-[hsl(var(--accent-error))]" />
        );
      }
    }

    // Offline state with has install (show play button regardless of selection)
    if ((state === 'offline' || state === 'uninstalled') && hasInstall) {
      return (
        <div className="relative w-full h-full flex items-center justify-center">
          {/* Circle background that appears on hover */}
          {isHovering && (
            <div
              className="absolute w-full h-full bg-[hsl(var(--launcher-accent-success))] rounded-full opacity-20"
              style={{
                boxShadow: `
                  0 0 4px 0px hsl(var(--launcher-accent-success) / 0.7),
                  0 0 8px 2px hsl(var(--launcher-accent-success) / 0.5),
                  0 0 12px 4px hsl(var(--launcher-accent-success) / 0.3),
                  0 0 16px 6px hsl(var(--launcher-accent-success) / 0.15),
                  0 0 20px 8px hsl(var(--launcher-accent-success) / 0.08),
                  0 0 24px 10px hsl(var(--launcher-accent-success) / 0.04)
                `
              }}
            />
          )}
          <Play
            className={`relative z-10 text-[hsl(var(--launcher-accent-success))] transition-all ${
              isHovering ? 'w-4 h-4' : 'w-3.5 h-3.5'
            }`}
            fill="currentColor"
            style={{
              filter: isHovering
                ? `drop-shadow(0 0 4px hsl(var(--launcher-accent-success) / 1))
                   drop-shadow(0 0 8px hsl(var(--launcher-accent-success) / 0.6))
                   drop-shadow(0 0 12px hsl(var(--launcher-accent-success) / 0.3))`
                : 'drop-shadow(0 0 6px hsl(var(--launcher-accent-success) / 0.6))'
            }}
          />
        </div>
      );
    }

    // Default small indicator for other states
    return null;
  };

  const content = getIndicatorContent();

  // Don't render if no content
  if (!content) {
    return null;
  }

  return (
    <div
      {...hoverProps}
      className="absolute right-0 top-1/2 transform translate-x-1/2 -translate-y-1/2 z-30 cursor-pointer"
      style={{
        width: indicatorRadius * 2,
        height: indicatorRadius * 2,
      }}
      onClick={handleClick}
    >
      {/* Invisible interaction zone */}
      <div className="absolute inset-0 rounded-full" />

      {/* Visible indicator */}
      <div className="w-full h-full flex items-center justify-center pointer-events-none">
        {content}
      </div>
    </div>
  );
};
