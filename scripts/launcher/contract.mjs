export const EXIT_CODES = Object.freeze({
  SUCCESS: 0,
  OPERATION_FAILED: 1,
  USAGE_ERROR: 2,
  MISSING_DEPENDENCY: 3,
  MISSING_RELEASE_ARTIFACT: 4,
});

export const ACTION_FLAGS = Object.freeze({
  HELP: '--help',
  INSTALL: '--install',
  BUILD: '--build',
  BUILD_RELEASE: '--build-release',
  RUN: '--run',
  RUN_RELEASE: '--run-release',
  TEST: '--test',
  RELEASE_SMOKE: '--release-smoke',
});

export const ACTION_NAMES = new Set(Object.values(ACTION_FLAGS));
export const PASSTHROUGH_ACTIONS = new Set([
  ACTION_FLAGS.RUN,
  ACTION_FLAGS.RUN_RELEASE,
]);

export function buildUsage(displayName = defaultDisplayName()) {
  return `Pumas Library launcher.

Usage:
  ${displayName} --help
  ${displayName} --install
  ${displayName} --build
  ${displayName} --build-release
  ${displayName} --run [-- <app args...>]
  ${displayName} --run-release [-- <app args...>]
  ${displayName} --test
  ${displayName} --release-smoke

Examples:
  ${displayName} --install
  ${displayName} --build
  ${displayName} --build-release
  ${displayName} --run -- --devtools
  ${displayName} --run-release -- --debug
  ${displayName} --test
  ${displayName} --release-smoke

Exit codes:
  0 success
  1 operation failed
  2 usage error
  3 missing dependency for runtime
  4 missing release artifact
`;
}

export function defaultDisplayName() {
  return process.env.PUMAS_LAUNCHER_DISPLAY_NAME ?? 'node scripts/launcher/cli.mjs';
}
