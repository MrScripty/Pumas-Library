import type { DesktopBridgeLinkMappingAPI } from './api-bridge-links';
import type { DesktopBridgeModelAPI } from './api-bridge-models';
import type { DesktopBridgeRuntimeAPI } from './api-bridge-runtime';
import type { DesktopBridgeUtilityAPI } from './api-bridge-utilities';

export type { DesktopBridgeLinkMappingAPI } from './api-bridge-links';
export type { DesktopBridgeModelAPI } from './api-bridge-models';
export type { DesktopBridgeRuntimeAPI } from './api-bridge-runtime';
export type { DesktopBridgeUtilityAPI } from './api-bridge-utilities';

/**
 * Canonical renderer bridge contract exposed by the Electron desktop shell.
 */
export interface DesktopBridgeAPI
  extends DesktopBridgeRuntimeAPI,
    DesktopBridgeModelAPI,
    DesktopBridgeLinkMappingAPI,
    DesktopBridgeUtilityAPI {}
