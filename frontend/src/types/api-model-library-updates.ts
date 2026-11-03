import type {
  ModelFactFamily as DesktopModelFactFamily,
  ModelLibraryChangeKind as DesktopModelLibraryChangeKind,
  ModelLibraryRefreshScope as DesktopModelLibraryRefreshScope,
  ModelLibraryUpdateEvent as DesktopModelLibraryUpdateEvent,
  ModelLibraryUpdateNotification as DesktopModelLibraryUpdateNotification,
} from '../generated/desktop-contract';

// Declared once by the backend (`/events/model-library-updates`) and generated;
// preload validates every pushed notification against that declaration.
export type ModelFactFamily = DesktopModelFactFamily;
export type ModelLibraryChangeKind = DesktopModelLibraryChangeKind;
export type ModelLibraryRefreshScope = DesktopModelLibraryRefreshScope;
export type ModelLibraryUpdateEvent = DesktopModelLibraryUpdateEvent;
export type ModelLibraryUpdateNotification = DesktopModelLibraryUpdateNotification;

export interface ModelLibraryUpdateFeed {
  cursor: string;
  events?: ModelLibraryUpdateEvent[];
  stale_cursor: boolean;
  snapshot_required: boolean;
}
