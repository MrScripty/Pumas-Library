import { ModelManager, type ModelManagerProps } from '../ModelManager';

export interface DefaultAppPanelProps {
  appDisplayName: string;
  modelManagerProps: ModelManagerProps;
}

export function DefaultAppPanel({
  appDisplayName,
  modelManagerProps,
}: DefaultAppPanelProps) {
  return (
    <div className="flex-1 flex flex-col gap-4 p-8 px-0 mx-2 py-1 overflow-hidden">
      <div className="text-center py-4">
        <p className="text-[hsl(var(--launcher-text-secondary))] text-sm">
          {`${appDisplayName} - Coming Soon`}
        </p>
      </div>

      <ModelManager {...modelManagerProps} />
    </div>
  );
}
