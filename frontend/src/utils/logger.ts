/**
 * Frontend logging utility using loglevel
 *
 * Provides structured logging interface matching backend patterns from backend/logging_config.py
 * Uses loglevel library for robust log level filtering and console output.
 *
 * Usage:
 *   import { getLogger } from '../utils/logger';
 *
 *   const logger = getLogger('AppIndicator');
 *   logger.info("Component mounted");
 *   logger.error("Failed to fetch data", error);
 */

import log from 'loglevel';

// Configure root logger
const rootLogger = log.getLogger('ComfyUILauncher');

// Set default log level based on environment
if (import.meta.env.MODE === 'development') {
  rootLogger.setLevel('debug');
} else {
  rootLogger.setLevel('info');
}

// Custom format for log messages to match backend style
const originalFactory = rootLogger.methodFactory;
rootLogger.methodFactory = function (methodName, logLevel, loggerName) {
  const rawMethod = originalFactory(methodName, logLevel, loggerName);

  return function (message: string, ...args: unknown[]) {
    const timestamp = new Date().toISOString().replace('T', ' ').slice(0, 19);
    const componentName = String(loggerName || 'Unknown');
    const level = String(methodName).toUpperCase();
    const prefix = `${timestamp} - ${componentName} - ${level}`;
    rawMethod(`${prefix} - ${message}`, ...args);
  };
};

// Apply the custom format
rootLogger.setLevel(rootLogger.getLevel());

/**
 * Get a logger instance for a component
 *
 * Creates a child logger with the component name for better tracking.
 * Inherits log level from root logger but can be configured independently.
 *
 * @param componentName - Name of the component (e.g., 'AppIndicator', 'VersionSelector')
 * @returns Logger instance configured for the component
 *
 * @example
 * const logger = getLogger('AppIndicator');
 * logger.debug('Hover state changed', { isHovered: true });
 * logger.info('State transition', { from: 'offline', to: 'running' });
 * logger.warn('Performance issue', { renderTime: 150 });
 * logger.error('Operation failed', error);
 */
export function getLogger(componentName: string) {
  const componentLogger = log.getLogger(componentName);

  // Inherit level from root logger
  componentLogger.setLevel(rootLogger.getLevel());

  // Apply custom format to component logger
  const originalFactory = componentLogger.methodFactory;
  componentLogger.methodFactory = function (methodName, logLevel, loggerName) {
    const rawMethod = originalFactory(methodName, logLevel, loggerName);

    return function (message: string, ...args: unknown[]) {
      const timestamp = new Date().toISOString().replace('T', ' ').slice(0, 19);
      const level = String(methodName).toUpperCase();
      const prefix = `${timestamp} - ${componentName} - ${level}`;
      rawMethod(`${prefix} - ${message}`, ...args);
    };
  };

  // Apply the format
  componentLogger.setLevel(componentLogger.getLevel());

  return componentLogger;
}

/**
 * Set global log level for all loggers
 *
 * @param level - Log level: 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'silent'
 *
 * @example
 * setLogLevel('warn'); // Only show warnings and errors
 */
export function setLogLevel(level: log.LogLevelDesc) {
  rootLogger.setLevel(level);
}

export default getLogger;
