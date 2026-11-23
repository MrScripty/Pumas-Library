import { BasaltPanel, type BasaltPanelProps } from './BasaltPanel';
import { DefaultAppPanel, type DefaultAppPanelProps } from './DefaultAppPanel';
import { LlamaCppPanel, type LlamaCppPanelProps } from './LlamaCppPanel';
import { OnnxRuntimePanel, type OnnxRuntimePanelProps } from './OnnxRuntimePanel';
import { OllamaPanel, type OllamaPanelProps } from './OllamaPanel';
import { TorchPanel, type TorchPanelProps } from './TorchPanel';
import { ModelManager } from '../ModelManager';

interface AppPanelRendererProps {
  selectedAppId: string | null;
  llamaCpp: LlamaCppPanelProps;
  onnxRuntime: OnnxRuntimePanelProps;
  basalt: BasaltPanelProps;
  ollama: OllamaPanelProps;
  torch: TorchPanelProps;
  fallback: DefaultAppPanelProps;
}

export function AppPanelRenderer({
  selectedAppId,
  llamaCpp,
  onnxRuntime,
  basalt,
  ollama,
  torch,
  fallback,
}: AppPanelRendererProps) {
  // No app selected - show Model Library as the default/home view
  if (!selectedAppId) {
    return (
      <div className="flex-1 flex flex-col overflow-hidden p-6">
        <ModelManager {...fallback.modelManagerProps} />
      </div>
    );
  }

  switch (selectedAppId) {
    case 'ollama':
      return <OllamaPanel {...ollama} />;
    case 'llama-cpp':
      return <LlamaCppPanel {...llamaCpp} />;
    case 'basalt':
      return <BasaltPanel {...basalt} />;
    case 'onnx-runtime':
      return <OnnxRuntimePanel {...onnxRuntime} />;
    case 'torch':
      return <TorchPanel {...torch} />;
    default:
      return <DefaultAppPanel {...fallback} />;
  }
}
