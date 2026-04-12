export class LauncherError extends Error {
  constructor(message, { exitCode = 1, showUsage = false } = {}) {
    super(message);
    this.name = 'LauncherError';
    this.exitCode = exitCode;
    this.showUsage = showUsage;
  }
}
