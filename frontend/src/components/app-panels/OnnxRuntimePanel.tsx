import type { ModelManagerProps } from '../ModelManager';
import { OnnxRuntimeModelLibrarySection } from './sections/OnnxRuntimeModelLibrarySection';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';

export interface OnnxRuntimePanelProps {
  modelManagerProps: ModelManagerProps;
}

export function OnnxRuntimePanel({ modelManagerProps }: OnnxRuntimePanelProps) {
  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      <RuntimeProfileSettingsSection provider="onnx_runtime" />
      <OnnxRuntimeModelLibrarySection
        excludedModels={modelManagerProps.excludedModels}
        modelGroups={modelManagerProps.modelGroups}
        servingEndpoint={modelManagerProps.servingEndpoint}
        servedModels={modelManagerProps.servedModels}
        starredModels={modelManagerProps.starredModels}
        onToggleLink={modelManagerProps.onToggleLink}
        onToggleStar={modelManagerProps.onToggleStar}
      />
    </div>
  );
}
