/**
 * Connection Info Section for GenericAppPanel.
 *
 * Displays the app's connection URL based on plugin configuration.
 */

import { AppConnectionInfo } from '../../AppConnectionInfo';
import type { ConnectionConfig } from '../../../types/plugins';

export interface ConnectionInfoSectionProps {
  connection?: ConnectionConfig;
  isRunning?: boolean;
  label?: string;
}

export function ConnectionInfoSection({
  connection,
  isRunning = false,
  label = 'Connection URL',
}: ConnectionInfoSectionProps) {
  if (!connection || !isRunning) {
    return null;
  }

  const url = `${connection.protocol}://localhost:${connection.defaultPort}`;

  return <AppConnectionInfo url={url} label={label} />;
}
