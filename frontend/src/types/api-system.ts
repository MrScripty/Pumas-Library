import type {
  StatusResponse as DesktopStatusResponse,
  StatusTelemetrySnapshot as DesktopStatusTelemetrySnapshot,
  SystemResources as DesktopSystemResources,
  StatusTelemetryUpdateNotification as DesktopStatusTelemetryUpdateNotification,
} from '@pumas/desktop-contract';
import type { BaseResponse } from './api-common';

// ============================================================================
// System & Status Types
// ============================================================================

export interface DiskSpaceResponse extends BaseResponse {
  total: number;
  used: number;
  free: number;
  percent: number;
}

// The backend's status and resource types, generated from its declaration.
export type StatusResponse = DesktopStatusResponse;
export type SystemResources = DesktopSystemResources;

export interface SystemResourcesResponse extends BaseResponse {
  resources: SystemResources;
}

// Declared once by the backend (`/events/status-telemetry-updates`) and generated.
export type StatusTelemetrySnapshot = DesktopStatusTelemetrySnapshot;
export type StatusTelemetryUpdateNotification = DesktopStatusTelemetryUpdateNotification;
