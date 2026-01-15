import { Zap, MessageSquare, Palette, Image, Cpu } from 'lucide-react';
import type { AppConfig } from '../types/apps';

/**
 * Default app configurations
 * This is the central registry for all supported applications
 */
export const DEFAULT_APPS: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'comfyui',
    displayName: 'ComfyUI',
    icon: Zap,
    status: 'idle',
    iconState: 'offline',
    description: 'Powerful and modular stable diffusion GUI',
    starred: false,
    linked: false,
  },
  {
    id: 'openwebui',
    name: 'openwebui',
    displayName: 'Open WebUI',
    icon: MessageSquare,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'User-friendly WebUI for LLMs',
    starred: false,
    linked: false,
  },
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
    id: 'invoke',
    name: 'invoke',
    displayName: 'InvokeAI',
    icon: Palette,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'Professional AI image generation',
    starred: false,
    linked: false,
  },
  {
    id: 'krita-diffusion',
    name: 'krita-diffusion',
    displayName: 'Krita Diffusion',
    icon: Image,
    status: 'idle',
    iconState: 'uninstalled',
    description: 'Stable Diffusion plugin for Krita',
    starred: false,
    linked: false,
  },
];

/**
 * Get app configuration by ID
 */
export function getAppById(id: string): AppConfig | undefined {
  return DEFAULT_APPS.find(app => app.id === id);
}

/**
 * Get default app (ComfyUI for now)
 */
export function getDefaultApp(): AppConfig {
  return DEFAULT_APPS[0]!;
}
