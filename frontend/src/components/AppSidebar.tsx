import React, { useState, useEffect, useRef } from 'react';
import { Plus, X } from 'lucide-react';
import { motion } from 'framer-motion';
import { createPortal } from 'react-dom';
import { AppIcon } from './AppIcon';
import { ComfyUIIcon } from './ComfyUIIcon';
import type { AppConfig } from '../types/apps';
import {
  usePhysicsDrag,
  ICON_SIZE,
  TOTAL_HEIGHT,
  LIST_TOP_PADDING
} from '../hooks/usePhysicsDrag';
import {
  useAnimationTimestamp,
  getDeleteZoneShakeStyle
} from '../utils/dragAnimations';

interface AppSidebarProps {
  apps: AppConfig[];
  selectedAppId: string | null;
  onSelectApp: (appId: string | null) => void;
  onSettingsClick?: () => void;
  onLaunchApp?: (appId: string) => void;
  onStopApp?: (appId: string) => void;
  onOpenLog?: (appId: string) => void;
  onDeleteApp?: (appId: string) => void;
  onReorderApps?: (reorderedApps: AppConfig[]) => void;
  onAddApp?: (insertAtIndex: number) => void;
}

export const AppSidebar: React.FC<AppSidebarProps> = ({
  apps,
  selectedAppId,
  onSelectApp,
  onLaunchApp,
  onStopApp,
  onOpenLog,
  onDeleteApp,
  onReorderApps,
  onAddApp,
}) => {
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });
  const [visualOrder, setVisualOrder] = useState<string[] | null>(null);
  const [settleBlend, setSettleBlend] = useState(0);
  const sidebarRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const timestamp = useAnimationTimestamp();
  const [portalRoot, setPortalRoot] = useState<HTMLElement | null>(null);
  const previewGapRef = useRef(0);
  const previewIndexRef = useRef<number | null>(null);

  const {
    draggedId,
    floatingId,
    floatingState,
    placeholderIndex,
    snapProximity,
    isInDeleteZone,
    deleteZoneShakeIntensity,
    dragX,
    dragY,
    dragOrigin,
    onPointerDown,
    completeDelete,
  } = usePhysicsDrag({
    apps,
    selectedAppId,
    onSelectApp,
    onReorderApps,
    onDeleteApp,
    listRef,
  });
  const isDragging = floatingState === 'dragging';
  const isSettling = floatingState === 'settling';
  const isDeleting = floatingState === 'deleting';

  // Track mouse position for Plus indicator
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      setMousePos({ x: e.clientX, y: e.clientY });
    };
    window.addEventListener('mousemove', handleMouseMove);
    return () => window.removeEventListener('mousemove', handleMouseMove);
  }, []);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const portal = document.createElement('div');
    portal.dataset.sidebarDragLayer = 'true';
    portal.style.position = 'fixed';
    portal.style.inset = '0';
    portal.style.pointerEvents = 'none';
    portal.style.zIndex = '9999';
    document.body.appendChild(portal);
    setPortalRoot(portal);
    return () => {
      portal.remove();
      setPortalRoot(null);
    };
  }, []);

  useEffect(() => {
    if (floatingState === 'dragging' && !visualOrder) {
      setVisualOrder(apps.map(app => app.id));
      return;
    }
    if (floatingState === null && visualOrder) {
      setVisualOrder(null);
    }
  }, [apps, floatingState, visualOrder]);

  useEffect(() => {
    if (!isSettling) {
      setSettleBlend(0);
      return;
    }
    let rafId = 0;
    const start = performance.now();
    const duration = 180;

    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = t * t * (3 - 2 * t);
      setSettleBlend(eased);
      if (t < 1) {
        rafId = requestAnimationFrame(tick);
      }
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [isSettling]);

  const getNearestIndex = () => {
    if (!listRef.current) return 0;
    const listRect = listRef.current.getBoundingClientRect();
    const relativeY = mousePos.y - listRect.top - LIST_TOP_PADDING;
    return Math.max(0, Math.min(apps.length, Math.round(relativeY / TOTAL_HEIGHT)));
  };

  // Calculate nearest position for Plus indicator
  const getNearestIconPosition = () => {
    if (!sidebarRef.current || !listRef.current) return 0;
    const listRect = listRef.current.getBoundingClientRect();
    const sidebarRect = sidebarRef.current.getBoundingClientRect();
    const listOffset = listRect.top - sidebarRect.top;
    const nearestIndex = getNearestIndex();
    return listOffset + LIST_TOP_PADDING + (nearestIndex * TOTAL_HEIGHT);
  };

  const handleSidebarClick = (e: React.MouseEvent) => {
    // Only deselect if clicking the background
    if (e.currentTarget === e.target && !draggedId && floatingState === null) {
      onSelectApp(null);
    }
  };

  const handleIconClick = (appId: string) => {
    if (!draggedId && floatingState === null) {
      onSelectApp(selectedAppId === appId ? null : appId);
    }
  };

  const handlePlusClick = () => {
    if (!draggedId && floatingState === null && onAddApp) {
      onAddApp(getNearestIndex());
    }
  };

  const listTransition = {
    type: 'spring',
    stiffness: 420,
    damping: 32
  };
  const listRect = listRef.current?.getBoundingClientRect();
  const floatingWidth = listRect?.width ?? ICON_SIZE;

  const appsById = new Map(apps.map(app => [app.id, app]));
  const orderedApps = visualOrder
    ? visualOrder.map(id => appsById.get(id)).filter((app): app is AppConfig => Boolean(app))
    : apps;
  const targetIndexById = new Map(apps.map((app, index) => [app.id, index]));
  const shouldExcludeFloating = isDragging || isSettling || isDeleting;
  const renderApps = shouldExcludeFloating && floatingId
    ? orderedApps.filter(app => app.id !== floatingId)
    : orderedApps;
  const draggedIndex = floatingId ? apps.findIndex(app => app.id === floatingId) : -1;
  const rawPlaceholderIndex = placeholderIndex ?? draggedIndex;
  const placeholderIndexCollapsed =
    draggedIndex >= 0 && rawPlaceholderIndex > draggedIndex
      ? rawPlaceholderIndex - 1
      : rawPlaceholderIndex;

  useEffect(() => {
    if (!isDragging) return;
    previewGapRef.current = snapProximity;
    previewIndexRef.current = placeholderIndexCollapsed;
  }, [isDragging, snapProximity, placeholderIndexCollapsed]);

  const floatingApp = floatingId ? apps.find(app => app.id === floatingId) : null;

  const renderAppIcon = (app: AppConfig, isFloating: boolean, offsetY: number) => {
    const hasInstall = app.iconState !== 'uninstalled';
    const launchError = app.iconState === 'error';
    const isOtherDragging = floatingId !== null && !isFloating;

    const shakeStyle: React.CSSProperties = (isFloating && deleteZoneShakeIntensity > 0.01)
      ? getDeleteZoneShakeStyle(deleteZoneShakeIntensity, timestamp)
      : {};

    const floatingStyle: React.CSSProperties = isFloating && dragOrigin
      ? {
          position: 'fixed',
          top: dragOrigin.y,
          left: dragOrigin.x,
          zIndex: 10000,
          width: floatingWidth
        }
      : {};

    const motionStyle = isFloating
      ? {
          x: dragX,
          y: dragY,
          scale: isDragging || isSettling ? 1.1 : 1,
          filter: isDragging || isSettling ? 'drop-shadow(0 12px 24px rgba(0,0,0,0.35))' : 'none',
          borderRadius: '9999px'
        }
      : {};

    const dragProps = {
      onPointerDown: (event: React.PointerEvent<HTMLElement>) =>
        onPointerDown(app.id, event),
    };

    const hideInList = !isFloating && isSettling && app.id === floatingId;

    const motionProps = {
      transition: isFloating && isDeleting
        ? { duration: 0.22, ease: 'easeIn' }
        : { ...listTransition, opacity: { duration: 0.12 } },
      style: {
        ...floatingStyle,
        ...motionStyle,
        touchAction: 'none',
        pointerEvents: hideInList ? 'none' : undefined,
      },
      animate: isFloating
        ? (isDeleting ? { scale: 0, rotate: -35, opacity: 0 } : undefined)
        : { y: offsetY, opacity: hideInList ? 0 : 1 },
      onAnimationComplete: () => {
        if (isFloating && isDeleting) {
          completeDelete();
        }
      },
      ...dragProps,
    };

    return (
      <motion.div key={app.id} {...motionProps}>
        <div className="relative">
          {app.id === 'comfyui' ? (
            <ComfyUIIcon
              state={app.iconState}
              isSelected={selectedAppId === app.id}
              onClick={() => handleIconClick(app.id)}
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
              disableShake={isOtherDragging}
            />
          ) : (
            <AppIcon
              appId={app.id}
              state={app.iconState}
              isSelected={selectedAppId === app.id}
              onClick={() => handleIconClick(app.id)}
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
              disableShake={isOtherDragging}
            />
          )}

          {isFloating && isDragging && isInDeleteZone && (
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <X
                className="w-8 h-8 text-accent-error"
                style={{ opacity: 0.4 + deleteZoneShakeIntensity * 0.6 }}
              />
            </div>
          )}
        </div>
      </motion.div>
    );
  };

  return (
    <div
      ref={sidebarRef}
      className="flex flex-col items-center p-3 gap-3 border-[hsl(var(--launcher-border))] transition-all duration-300 relative py-1 h-auto font-normal font-mono shadow-none border-r-0 mx-0 px-1 w-16 overflow-visible bg-[hsl(var(--launcher-bg-secondary)/0.5)]"
      onClick={handleSidebarClick}
    >
      {/* Plus indicator - show on hover when not dragging */}
      {!draggedId && floatingState === null && mousePos.y > 0 && sidebarRef.current && (
        <div
          className="absolute left-1/2 transform -translate-x-1/2 z-0 opacity-50 hover:opacity-100 transition-opacity cursor-pointer"
          style={{ top: `${getNearestIconPosition()}px` }}
          onClick={handlePlusClick}
        >
          <Plus className="w-8 h-8 text-[hsl(var(--launcher-accent-primary)/0.5)]" />
        </div>
      )}

      {/* Icon list with Framer Motion */}
      <div ref={listRef} className="flex flex-col gap-3 w-full relative z-10 pt-3">
        {renderApps.map((app, baseIndex) => {
          let offsetY = 0;
          const previewIndex = previewIndexRef.current ?? placeholderIndexCollapsed;
          const previewGap = isDragging ? snapProximity : previewGapRef.current;
          const previewOffset =
            previewIndex !== null && baseIndex >= previewIndex
              ? TOTAL_HEIGHT * previewGap
              : 0;

          if (isDragging && draggedIndex >= 0 && rawPlaceholderIndex !== null) {
            offsetY = previewOffset;
          } else if (isSettling && visualOrder) {
            const targetIndex = targetIndexById.get(app.id);
            const settleOffset = typeof targetIndex === 'number'
              ? (targetIndex - baseIndex) * TOTAL_HEIGHT
              : 0;
            offsetY = previewOffset + (settleOffset - previewOffset) * settleBlend;
          }
          return renderAppIcon(app, false, offsetY);
        })}
      </div>

      {portalRoot && floatingApp ? createPortal(renderAppIcon(floatingApp, true, 0), portalRoot) : null}
    </div>
  );
};
