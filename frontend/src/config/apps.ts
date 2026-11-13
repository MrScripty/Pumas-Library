import { Cpu, Flame, CircuitBoard } from 'lucide-react';
import type { AppConfig } from '../types/apps';

/**
 * Default app configurations
 * This is the central registry for all supported applications
 */
export const INFERENCE_PLUGIN_APPS: AppConfig[] = [
  {
    id: 'ollama',
    name: 'ollama',
    displayName: 'Ollama',
    icon: Cpu,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'Local LLM runtime and model server',
    connectionUrl: 'http://localhost:11434',
    starred: false,
    linked: false,
  },
  {
    id: 'llama-cpp',
    name: 'llama-cpp',
    displayName: 'llama.cpp',
    icon: CircuitBoard,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'Native GGUF runtime and OpenAI-compatible model server',
    starred: false,
    linked: false,
  },
  {
    id: 'onnx-runtime',
    name: 'onnx-runtime',
    displayName: 'ONNX Runtime',
    icon: Cpu,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'Downloaded ONNX Runtime, hosted per profile, for ONNX embeddings',
    starred: false,
    linked: false,
  },
  {
    id: 'torch',
    name: 'torch',
    displayName: 'Torch',
    icon: Flame,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'PyTorch inference engine with OpenAI-compatible API',
    connectionUrl: 'http://localhost:8400',
    starred: false,
    linked: false,
  },
];

export const DEFAULT_APPS: AppConfig[] = __FEATURE_INFERENCE_PLUGINS__
  ? INFERENCE_PLUGIN_APPS
  : [];

/**
 * Get app configuration by ID
 */
export function getAppById(id: string): AppConfig | undefined {
  return DEFAULT_APPS.find(app => app.id === id);
}

/**
 * Get the first compiled-in inference plugin.
 */
export function getDefaultApp(): AppConfig {
  const defaultApp = DEFAULT_APPS[0];
  if (defaultApp === undefined) {
    throw new TypeError('DEFAULT_APPS must include at least one app');
  }

  return defaultApp;
}
