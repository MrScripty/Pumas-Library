/**
 * Theme configuration for the ComfyUI Launcher
 *
 * This file provides type-safe access to theme colors throughout the application.
 * Colors are defined as HSL values in CSS custom properties (see index.css).
 *
 * Usage:
 * import { themeColors } from '@/config/theme';
 * className={`bg-[${themeColors.surfaces.interactive}]`}
 */

import { getLogger } from '../utils/logger';

const logger = getLogger('theme');

export const themeColors = {
  /**
   * Surface colors - Used for backgrounds at different elevation levels
   * Lower surfaces are darker, higher surfaces are lighter
   */
  surfaces: {
    /** Lowest level - main app background */
    lowest: 'hsl(var(--surface-lowest))',
    /** Low level - elevated sections */
    low: 'hsl(var(--surface-low))',
    /** Mid level - cards and panels */
    mid: 'hsl(var(--surface-mid))',
    /** High level - controls and inputs */
    high: 'hsl(var(--surface-high))',
    /** Highest level - emphasized elements */
    highest: 'hsl(var(--surface-highest))',
    /** Overlay - dropdowns, modals, tooltips */
    overlay: 'hsl(var(--surface-overlay))',
    /** Interactive - buttons, controls, selects */
    interactive: 'hsl(var(--surface-interactive))',
    /** Interactive hover state */
    interactiveHover: 'hsl(var(--surface-interactive-hover))',
  },

  /**
   * Text colors - Hierarchy from most to least prominent
   */
  text: {
    /** Primary text - headings, important content */
    primary: 'hsl(var(--text-primary))',
    /** Secondary text - body text, labels */
    secondary: 'hsl(var(--text-secondary))',
    /** Tertiary text - muted, disabled, placeholders */
    tertiary: 'hsl(var(--text-tertiary))',
  },

  /**
   * Border colors
   */
  borders: {
    /** Default border color */
    default: 'hsl(var(--border-default))',
    /** Control border - for inputs, buttons, etc. */
    control: 'hsl(var(--border-control))',
  },

  /**
   * Accent colors - Semantic colors for states and actions
   */
  accent: {
    /** Success - completed actions, active states */
    success: 'hsl(var(--accent-success))',
    /** Error - failures, destructive actions */
    error: 'hsl(var(--accent-error))',
    /** Info - informational messages, system status */
    info: 'hsl(var(--accent-info))',
    /** Link - clickable links, connections */
    link: 'hsl(var(--accent-link))',
    /** Warning - caution, important notices */
    warning: 'hsl(var(--accent-warning))',
  },

  /**
   * Raw launcher tokens - Direct access to launcher-prefixed variables
   * Use semantic colors above when possible for better maintainability
   */
  launcher: {
    background: {
      primary: 'hsl(var(--launcher-bg-primary))',
      secondary: 'hsl(var(--launcher-bg-secondary))',
      tertiary: 'hsl(var(--launcher-bg-tertiary))',
      control: 'hsl(var(--launcher-bg-control))',
      controlHover: 'hsl(var(--launcher-bg-control-hover))',
      elevated: 'hsl(var(--launcher-bg-elevated))',
      overlay: 'hsl(var(--launcher-bg-overlay))',
    },
    text: {
      primary: 'hsl(var(--launcher-text-primary))',
      secondary: 'hsl(var(--launcher-text-secondary))',
      muted: 'hsl(var(--launcher-text-muted))',
    },
    border: {
      default: 'hsl(var(--launcher-border))',
      control: 'hsl(var(--launcher-border-control))',
    },
    accent: {
      primary: 'hsl(var(--launcher-accent-primary))',
      error: 'hsl(var(--launcher-accent-error))',
      success: 'hsl(var(--launcher-accent-success))',
      info: 'hsl(var(--launcher-accent-info))',
      link: 'hsl(var(--launcher-accent-link))',
      warning: 'hsl(var(--launcher-accent-warning))',
      cpu: 'hsl(var(--launcher-accent-cpu))',
      ram: 'hsl(var(--launcher-accent-ram))',
      gpu: 'hsl(var(--launcher-accent-gpu))',
    },
  },
} as const;

/**
 * Utility class names for common theme patterns
 * These correspond to the @layer components in index.css
 */
export const themeClasses = {
  surfaces: {
    lowest: 'surface-lowest',
    low: 'surface-low',
    mid: 'surface-mid',
    high: 'surface-high',
    highest: 'surface-highest',
    overlay: 'surface-overlay',
    interactive: 'surface-interactive',
    interactiveHover: 'surface-interactive-hover',
  },
  text: {
    primary: 'text-primary',
    secondary: 'text-secondary',
    tertiary: 'text-tertiary',
    accentSuccess: 'text-accent-success',
    accentError: 'text-accent-error',
    accentInfo: 'text-accent-info',
    accentLink: 'text-accent-link',
    accentWarning: 'text-accent-warning',
  },
  backgrounds: {
    accentSuccess: 'bg-accent-success',
    accentError: 'bg-accent-error',
    accentInfo: 'bg-accent-info',
    accentLink: 'bg-accent-link',
    accentWarning: 'bg-accent-warning',
  },
  borders: {
    default: 'border-default',
    control: 'border-control',
  },
} as const;

/**
 * Helper function to get a theme color value
 * Useful for dynamic color selection
 */
export function getThemeColor(path: string): string {
  const parts = path.split('.');
  let value: Record<string, unknown> | string = themeColors as unknown as Record<string, unknown>;

  for (const part of parts) {
    if (value && typeof value === 'object' && part in value) {
      value = value[part] as Record<string, unknown> | string;
    } else {
      logger.warn('Theme color path not found', { path });
      return 'hsl(0 0% 50%)'; // Fallback gray
    }
  }

  return typeof value === 'string' ? value : 'hsl(0 0% 50%)';
}

/**
 * Type-safe theme color paths for autocomplete
 * Example: 'surfaces.interactive', 'text.primary', 'accent.success'
 */
export type ThemeColorPath =
  | `surfaces.${keyof typeof themeColors.surfaces}`
  | `text.${keyof typeof themeColors.text}`
  | `borders.${keyof typeof themeColors.borders}`
  | `accent.${keyof typeof themeColors.accent}`;
