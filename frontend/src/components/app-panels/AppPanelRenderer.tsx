import { ComfyUIPanel, type ComfyUIPanelProps } from './ComfyUIPanel';
import { DefaultAppPanel, type DefaultAppPanelProps } from './DefaultAppPanel';
import { OllamaPanel, type OllamaPanelProps } from './OllamaPanel';
import { ModelManager } from '../ModelManager';

interface AppPanelRendererProps {
  selectedAppId: string | null;
  comfyUI: ComfyUIPanelProps;
  ollama: OllamaPanelProps;
  fallback: DefaultAppPanelProps;
}

export function AppPanelRenderer({
  selectedAppId,
  comfyUI,
  ollama,
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
    case 'comfyui':
      return <ComfyUIPanel {...comfyUI} />;
    case 'ollama':
      return <OllamaPanel {...ollama} />;
    default:
      return <DefaultAppPanel {...fallback} />;
  }
}
