export function log(message) {
  process.stdout.write(`[launcher] ${message}\n`);
}

export function logError(message) {
  process.stderr.write(`[launcher] ${message}\n`);
}
