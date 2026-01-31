/**
 * Model Library Section for GenericAppPanel.
 *
 * Embeds the ModelManager component within an app panel.
 */

import { ModelManager, type ModelManagerProps } from '../../ModelManager';

export interface ModelLibrarySectionProps extends ModelManagerProps {
  /** Whether to show the section */
  enabled?: boolean;
}

export function ModelLibrarySection({
  enabled = true,
  ...modelManagerProps
}: ModelLibrarySectionProps) {
  if (!enabled) {
    return null;
  }

  return (
    <div className="flex-1 min-h-0 overflow-hidden">
      <ModelManager {...modelManagerProps} />
    </div>
  );
}
