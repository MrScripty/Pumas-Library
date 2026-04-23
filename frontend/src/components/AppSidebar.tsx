import React, { useState, useEffect, useRef } from 'react';
import { Plus } from 'lucide-react';
import { createPortal } from 'react-dom';
import type { AppConfig } from '../types/apps';
import {
  usePhysicsDrag,
  ICON_SIZE,
  TOTAL_HEIGHT,
  LIST_TOP_PADDING
} from '../hooks/usePhysicsDrag';
import {
  useAnimationTimestamp,
} from '../utils/dragAnimations';
import { Tooltip } from './ui';
import { SidebarAppIcon } from './SidebarAppIcon';

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
    portal.dataset['sidebarDragLayer'] = 'true';
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

  const handleSidebarPointerDown = (e: React.PointerEvent) => {
    // Only deselect if clicking the background
    if (e.currentTarget === e.target && !draggedId && floatingState === null) {
      onSelectApp(null);
    }
  };

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && selectedAppId && !draggedId && floatingState === null) {
        onSelectApp(null);
      }
    };

    window.addEventListener('keydown', handleEscape);
    return () => window.removeEventListener('keydown', handleEscape);
  }, [draggedId, floatingState, onSelectApp, selectedAppId]);

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

  const renderAppIcon = (app: AppConfig, isFloating: boolean, offsetY: number) => (
    <SidebarAppIcon
      key={app.id}
      app={app}
      completeDelete={completeDelete}
      deleteZoneShakeIntensity={deleteZoneShakeIntensity}
      dragOrigin={dragOrigin}
      dragX={dragX}
      dragY={dragY}
      floatingId={floatingId}
      floatingWidth={floatingWidth}
      isDeleting={isDeleting}
      isDragging={isDragging}
      isFloating={isFloating}
      isInDeleteZone={isInDeleteZone}
      isSettling={isSettling}
      offsetY={offsetY}
      selectedAppId={selectedAppId}
      timestamp={timestamp}
      onIconClick={handleIconClick}
      onLaunchApp={onLaunchApp}
      onOpenLog={onOpenLog}
      onPointerDown={onPointerDown}
      onStopApp={onStopApp}
    />
  );

  return (
    <div
      ref={sidebarRef}
      className="flex flex-col items-center p-3 gap-3 border-[hsl(var(--launcher-border))] transition-all duration-300 relative py-1 h-auto font-normal font-mono shadow-none border-r-0 mx-0 px-1 w-16 overflow-visible bg-[hsl(var(--launcher-bg-secondary)/0.5)]"
      onPointerDown={handleSidebarPointerDown}
      role="toolbar"
      tabIndex={-1}
    >
      {/* Plus indicator - show on hover when not dragging */}
      {!draggedId && floatingState === null && mousePos.y > 0 && sidebarRef.current && (
        <Tooltip content="Add app" position="right">
          <button
            type="button"
            className="absolute left-1/2 transform -translate-x-1/2 z-0 opacity-50 hover:opacity-100 transition-opacity cursor-pointer bg-transparent border-0 p-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[hsl(var(--accent-primary))]"
            style={{ top: `${getNearestIconPosition()}px` }}
            onClick={handlePlusClick}
            aria-label="Add app"
          >
            <Plus className="w-8 h-8 text-[hsl(var(--accent-primary)/0.5)]" />
          </button>
        </Tooltip>
      )}

      {/* Icon list with Framer Motion */}
      <div ref={listRef} className="flex flex-col gap-3 w-full relative z-10 pt-3">
        {renderApps.map((app, baseIndex) => {
          let offsetY = 0;
          const previewIndex = previewIndexRef.current ?? placeholderIndexCollapsed;
          const previewGap = isDragging ? snapProximity : previewGapRef.current;
          const previewOffset =
            baseIndex >= previewIndex
              ? TOTAL_HEIGHT * previewGap
              : 0;

          if (isDragging && draggedIndex >= 0) {
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
