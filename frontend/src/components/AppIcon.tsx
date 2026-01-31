import React from 'react';
import type { AppIconState } from '../types/apps';
import { useSmoothResource } from '../hooks/useSmoothResource';
import { AppIndicator } from './AppIndicator';

interface AppIconProps {
  appId: string;
  state: AppIconState;
  isSelected?: boolean;
  onClick?: () => void;
  title?: string;
  ramUsage?: number;
  gpuUsage?: number;
  hasInstall?: boolean;
  launchError?: boolean;
  onLaunch?: () => void;
  onStop?: () => void;
  onOpenLog?: () => void;
  dragOpacity?: number;
  shakeStyle?: React.CSSProperties;
  _disableShake?: boolean;
  isGhost?: boolean;
}

interface IconWrapperProps {
  isSelected: boolean;
  children: React.ReactNode;
}

const IconWrapper: React.FC<IconWrapperProps> = ({ isSelected, children }) => {
  return (
    <div className="relative w-full aspect-square">
      {isSelected && (
        <div
          className="absolute inset-0 rounded-full transition-all duration-300 ease-in-out"
          style={{
            filter: 'blur(12px)',
            background: 'radial-gradient(circle, hsl(var(--launcher-accent-primary)/0.6) 0%, hsl(var(--launcher-accent-primary)/0.3) 50%, transparent 70%)',
            transform: 'scale(1.1)',
            zIndex: -1
          }}
        />
      )}
      <div className="relative z-10">
        {children}
      </div>
    </div>
  );
};

const getIconPath = (appId: string): string => {
  const iconMap: Record<string, string> = {
    'openwebui': './openwebui-icon.png',
    'ollama': './ollama-icon.png',
    'invoke': './invoke-icon.png',
    'krita-diffusion': './krita-diffusion-icon.png',
  };
  return iconMap[appId] || './comfyui-icon.png';
};

const RunningIcon: React.FC<{ appId: string; ramUsage?: number; gpuUsage?: number; isSelected?: boolean }> = ({
  appId,
  ramUsage = 60,
  gpuUsage = 40,
  isSelected = false
}) => {
  // Smooth interpolation between updates
  const smoothRam = useSmoothResource(ramUsage, 500) ?? ramUsage;
  const smoothGpu = useSmoothResource(gpuUsage, 500) ?? gpuUsage;

  const iconSize = 60;
  const centerX = iconSize / 2;
  const centerY = iconSize / 2;
  const radius = iconSize / 2 - 2;

  const ramAngle = (smoothRam / 100) * Math.PI;
  const gpuAngle = (smoothGpu / 100) * Math.PI;

  const ramEndX = centerX + radius * Math.cos(Math.PI - ramAngle);
  const ramEndY = centerY + radius * Math.sin(Math.PI - ramAngle);

  const gpuEndX = centerX + radius * Math.cos(Math.PI + gpuAngle);
  const gpuEndY = centerY + radius * Math.sin(Math.PI + gpuAngle);

  return (
    <IconWrapper isSelected={isSelected}>
      <div className="w-full aspect-square bg-[hsl(var(--launcher-bg-tertiary)/0.3)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors flex items-center justify-center border-[hsl(var(--launcher-border))] hover:border-[hsl(var(--launcher-border)/0.8)] cursor-pointer relative border-2 rounded-full">
        <div className="absolute inset-0 rounded-full overflow-hidden">
          <img
            src={getIconPath(appId)}
            alt={appId}
            className="w-full h-full absolute inset-0 object-cover"
            style={{ clipPath: 'circle(48% at 50% 50%)' }}
            draggable={false}
          />
        </div>

        <svg
          className="absolute inset-0 z-20"
          viewBox={`0 0 ${iconSize} ${iconSize}`}
          style={{ width: '100%', height: '100%', opacity: 1 }}
        >
          <defs>
            <filter id={`ram-glow-${appId}`}>
              <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="blur" />
              <feComposite in="SourceGraphic" in2="blur" operator="over" />
            </filter>
            <filter id={`gpu-glow-${appId}`}>
              <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="blur" />
              <feComposite in="SourceGraphic" in2="blur" operator="over" />
            </filter>
          </defs>
          <path
            d={`M ${centerX - radius} ${centerY} A ${radius} ${radius} 0 0 0 ${ramEndX} ${ramEndY}`}
            stroke="hsl(var(--launcher-accent-ram))"
            strokeWidth="3"
            fill="none"
            strokeLinecap="round"
            filter={`url(#ram-glow-${appId})`}
            opacity="1"
          />
          <path
            d={`M ${centerX - radius} ${centerY} A ${radius} ${radius} 0 0 1 ${gpuEndX} ${gpuEndY}`}
            stroke="hsl(var(--launcher-accent-gpu))"
            strokeWidth="3"
            fill="none"
            strokeLinecap="round"
            filter={`url(#gpu-glow-${appId})`}
            opacity="1"
          />
        </svg>
      </div>
    </IconWrapper>
  );
};

const OfflineIcon: React.FC<{ appId: string; isSelected?: boolean }> = ({ appId, isSelected = false }) => {
  return (
    <IconWrapper isSelected={isSelected}>
      <div className="w-full aspect-square bg-[hsl(var(--launcher-bg-tertiary)/0.3)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors flex items-center justify-center border-[hsl(var(--launcher-border))] hover:border-[hsl(var(--launcher-border)/0.8)] cursor-pointer relative border-2 rounded-full opacity-80">
        <div className="absolute inset-0 rounded-full overflow-hidden">
          <img
            src={getIconPath(appId)}
            alt={appId}
            className="w-full h-full object-cover"
            style={{ clipPath: 'circle(48% at 50% 50%)' }}
            draggable={false}
          />
        </div>
      </div>
    </IconWrapper>
  );
};

const ErrorIcon: React.FC<{ appId: string; isSelected?: boolean }> = ({ appId, isSelected = false }) => {
  return (
    <IconWrapper isSelected={isSelected}>
      <div className="w-full aspect-square bg-[hsl(var(--launcher-bg-tertiary)/0.3)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors flex items-center justify-center border-[hsl(var(--launcher-border))] hover:border-[hsl(var(--launcher-border)/0.8)] cursor-pointer relative border-2 rounded-full opacity-80">
        <div className="absolute inset-0 rounded-full overflow-hidden">
          <img
            src={getIconPath(appId)}
            alt={appId}
            className="w-full h-full object-cover"
            style={{ clipPath: 'circle(48% at 50% 50%)' }}
            draggable={false}
          />
        </div>
      </div>
    </IconWrapper>
  );
};

const UninstalledIcon: React.FC<{ appId: string; isSelected?: boolean }> = ({ appId, isSelected = false }) => {
  return (
    <IconWrapper isSelected={isSelected}>
      <div className="w-full aspect-square bg-[hsl(var(--launcher-bg-tertiary)/0.3)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors flex items-center justify-center border-[hsl(var(--launcher-border))] hover:border-[hsl(var(--launcher-border)/0.8)] cursor-pointer relative border-2 rounded-full opacity-60">
        <div className="absolute inset-0 rounded-full overflow-hidden">
          <img
            src={getIconPath(appId)}
            alt={appId}
            className="w-full h-full object-cover"
            style={{ clipPath: 'circle(48% at 50% 50%)' }}
            draggable={false}
          />
        </div>
      </div>
    </IconWrapper>
  );
};

export const AppIcon: React.FC<AppIconProps> = ({
  appId,
  state,
  isSelected = false,
  onClick,
  title,
  ramUsage,
  gpuUsage,
  hasInstall = false,
  launchError = false,
  onLaunch,
  onStop,
  onOpenLog,
  dragOpacity = 1.0,
  shakeStyle = {},
  _disableShake = false,
  isGhost = false,
}) => {
  return (
    <button
      onClick={onClick}
      onDragStart={(event) => event.preventDefault()}
      className={`relative w-full overflow-visible hover:z-50 bg-transparent border-0 p-0 outline-none appearance-none select-none ${
        isGhost ? 'shadow-2xl cursor-grabbing' : 'cursor-grab'
      } ${isSelected ? 'opacity-100' : state === 'uninstalled' ? 'opacity-60' : 'opacity-80'}`}
      style={{
        opacity: dragOpacity,
        transition: 'opacity 0.2s ease-out',
        ...shakeStyle
      }}
      title={title}
    >
      <div className="relative overflow-visible">
        {state === 'running' ? (
          <RunningIcon appId={appId} ramUsage={ramUsage} gpuUsage={gpuUsage} isSelected={isSelected} />
        ) : state === 'starting' ? (
          <div className="animate-pulse">
            <OfflineIcon appId={appId} isSelected={isSelected} />
          </div>
        ) : state === 'stopping' ? (
          <div className="animate-pulse">
            <RunningIcon appId={appId} ramUsage={ramUsage} gpuUsage={gpuUsage} isSelected={isSelected} />
          </div>
        ) : state === 'offline' ? (
          <OfflineIcon appId={appId} isSelected={isSelected} />
        ) : state === 'uninstalled' ? (
          <UninstalledIcon appId={appId} isSelected={isSelected} />
        ) : (
          <ErrorIcon appId={appId} isSelected={isSelected} />
        )}

        {!isGhost && (
          <AppIndicator
            appId={appId}
            state={state}
            isSelected={isSelected}
            hasInstall={hasInstall}
            launchError={launchError}
            onLaunch={onLaunch}
            onStop={onStop}
            onOpenLog={onOpenLog}
          />
        )}
      </div>
    </button>
  );
};
