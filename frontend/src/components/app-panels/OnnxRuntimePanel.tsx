import { useMemo } from 'react';
import { ModelManager, type ModelManagerProps } from '../ModelManager';
import { filterProviderCompatibleModelGroups } from '../../utils/runtimeProviderDescriptors';
import { RuntimeProfileSettingsSection } from './sections/RuntimeProfileSettingsSection';

export interface OnnxRuntimePanelProps {
  modelManagerProps: ModelManagerProps;
}

export function OnnxRuntimePanel({ modelManagerProps }: OnnxRuntimePanelProps) {
  const onnxModelGroups = useMemo(
    () => filterProviderCompatibleModelGroups(modelManagerProps.modelGroups, 'onnx_runtime'),
    [modelManagerProps.modelGroups]
  );

  return (
    <div className="flex-1 flex flex-col gap-4 p-6 overflow-hidden">
      <RuntimeProfileSettingsSection provider="onnx_runtime" />
      <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
        <ModelManager {...modelManagerProps} modelGroups={onnxModelGroups} />
      </section>
    </div>
  );
}
