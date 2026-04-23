import React from 'react';
import { X } from 'lucide-react';
import { motion } from 'framer-motion';
import type { MotionValue } from 'framer-motion';
import { AppIcon } from './AppIcon';
import { ComfyUIIcon } from './ComfyUIIcon';
import type { AppConfig } from '../types/apps';
import { getDeleteZoneShakeStyle } from '../utils/dragAnimations';

interface SidebarAppIconProps {
  app: AppConfig;
  completeDelete: () => void;
  deleteZoneShakeIntensity: number;
  dragOrigin: { x: number; y: number } | null;
  dragX: MotionValue<number>;
  dragY: MotionValue<number>;
  floatingId: string | null;
  floatingWidth: number;
  isDeleting: boolean;
  isDragging: boolean;
  isFloating: boolean;
  isInDeleteZone: boolean;
  isSettling: boolean;
  offsetY: number;
  selectedAppId: string | null;
  timestamp: number;
  onIconClick: (appId: string) => void;
  onLaunchApp?: (appId: string) => void;
  onOpenLog?: (appId: string) => void;
  onPointerDown: (appId: string, event: React.PointerEvent<HTMLElement>) => void;
  onStopApp?: (appId: string) => void;
}

const listTransition = {
  type: 'spring',
  stiffness: 420,
  damping: 32,
};

export function SidebarAppIcon({
  app,
  completeDelete,
  deleteZoneShakeIntensity,
  dragOrigin,
  dragX,
  dragY,
  floatingId,
  floatingWidth,
  isDeleting,
  isDragging,
  isFloating,
  isInDeleteZone,
  isSettling,
  offsetY,
  selectedAppId,
  timestamp,
  onIconClick,
  onLaunchApp,
  onOpenLog,
  onPointerDown,
  onStopApp,
}: SidebarAppIconProps) {
  const hasInstall = app.iconState !== 'uninstalled';
  const launchError = app.iconState === 'error';
  const isOtherDragging = floatingId !== null && !isFloating;
  const hideInList = !isFloating && isSettling && app.id === floatingId;

  const shakeStyle: React.CSSProperties = (isFloating && deleteZoneShakeIntensity > 0.01)
    ? getDeleteZoneShakeStyle(deleteZoneShakeIntensity, timestamp)
    : {};

  const floatingStyle: React.CSSProperties = isFloating && dragOrigin
    ? {
        position: 'fixed',
        top: dragOrigin.y,
        left: dragOrigin.x,
        zIndex: 10000,
        width: floatingWidth,
      }
    : {};

  const motionStyle = isFloating
    ? {
        x: dragX,
        y: dragY,
        scale: isDragging || isSettling ? 1.1 : 1,
        filter: isDragging || isSettling ? 'drop-shadow(0 12px 24px rgba(0,0,0,0.35))' : 'none',
        borderRadius: '9999px',
      }
    : {};

  const motionProps = {
    transition: isFloating && isDeleting
      ? { duration: 0.22, ease: 'easeIn' }
      : { ...listTransition, opacity: { duration: 0.12 } },
    style: {
      ...floatingStyle,
      ...motionStyle,
      touchAction: 'none' as const,
      pointerEvents: hideInList ? 'none' as const : undefined,
    },
    animate: isFloating
      ? (isDeleting ? { scale: 0, rotate: -35, opacity: 0 } : undefined)
      : { y: offsetY, opacity: hideInList ? 0 : 1 },
    onAnimationComplete: () => {
      if (isFloating && isDeleting) {
        completeDelete();
      }
    },
    onPointerDown: (event: React.PointerEvent<HTMLElement>) => onPointerDown(app.id, event),
  };

  return (
    // @ts-expect-error - Framer Motion transition type incompatibility
    <motion.div key={app.id} {...motionProps}>
      <div className="relative">
        {app.id === 'comfyui' ? (
          <ComfyUIIcon
            state={app.iconState}
            isSelected={selectedAppId === app.id}
            onClick={() => onIconClick(app.id)}
            title={app.displayName}
            ramUsage={app.ramUsage}
            gpuUsage={app.gpuUsage}
            hasInstall={hasInstall}
            launchError={launchError}
            onLaunch={() => onLaunchApp?.(app.id)}
            onStop={() => onStopApp?.(app.id)}
            onOpenLog={() => onOpenLog?.(app.id)}
            dragOpacity={1.0}
            shakeStyle={shakeStyle}
            _disableShake={isOtherDragging}
          />
        ) : (
          <AppIcon
            appId={app.id}
            state={app.iconState}
            isSelected={selectedAppId === app.id}
            onClick={() => onIconClick(app.id)}
            title={app.displayName}
            ramUsage={app.ramUsage}
            gpuUsage={app.gpuUsage}
            hasInstall={hasInstall}
            launchError={launchError}
            onLaunch={() => onLaunchApp?.(app.id)}
            onStop={() => onStopApp?.(app.id)}
            onOpenLog={() => onOpenLog?.(app.id)}
            dragOpacity={1.0}
            shakeStyle={shakeStyle}
            _disableShake={isOtherDragging}
          />
        )}

        {isFloating && isDragging && isInDeleteZone && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <X
              className="h-8 w-8 text-accent-error"
              style={{ opacity: 0.4 + deleteZoneShakeIntensity * 0.6 }}
            />
          </div>
        )}
      </div>
    </motion.div>
  );
}
