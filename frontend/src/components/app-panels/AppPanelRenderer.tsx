import { ComfyUIPanel, type ComfyUIPanelProps } from './ComfyUIPanel';
import { DefaultAppPanel, type DefaultAppPanelProps } from './DefaultAppPanel';
import { OllamaPanel, type OllamaPanelProps } from './OllamaPanel';

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
  if (!selectedAppId) {
    return null;
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
