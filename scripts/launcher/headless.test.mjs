import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const actionsUrl = new URL('./actions.mjs', import.meta.url).href;
const nativeFixture = { skip: process.platform === 'win32' && 'fixture uses native Unix executable scripts' };

// Real launcher/process delegation with isolated recording executables. These
// prove command selection and lifecycle, not Rust compilation or GUI behavior.
function fixture(t, { desktop = false } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pumas-launcher-headless-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const events = path.join(root, 'events.jsonl');
  const executable = (name) => {
    const filename = path.join(root, name);
    fs.writeFileSync(filename, `#!${process.execPath}
import fs from 'node:fs';
fs.appendFileSync(process.env.PUMAS_TEST_EVENTS, JSON.stringify({
  executable: ${JSON.stringify(name)}, args: process.argv.slice(2),
  plugins: process.env.PUMAS_INFERENCE_PLUGINS
}) + '\\n');
process.exit(Number(process.env.PUMAS_TEST_CHILD_EXIT || 0));
`);
    fs.chmodSync(filename, 0o755);
    return filename;
  };
  const context = {
    repoRoot: root, displayName: './launcher.sh', appBin: 'pumas-rpc',
    rustManifestPath: path.join(root, 'rust', 'Cargo.toml'),
    frontendDir: path.join(root, 'frontend'), electronDir: path.join(root, 'electron'),
    frontendDistIndex: path.join(root, 'frontend', 'dist', 'index.html'),
    electronDistMain: path.join(root, 'electron', 'dist', 'main.js'),
  };
  const paths = {
    cargo: executable('cargo.mjs'), corepack: executable('corepack.mjs'),
    debug: executable('debug-backend.mjs'), release: executable('release-backend.mjs'),
  };
  if (desktop) {
    for (const directory of [root, context.frontendDir, context.electronDir]) {
      fs.mkdirSync(path.join(directory, 'node_modules'), { recursive: true });
    }
  }
  const launcherTests = path.join(root, 'scripts', 'launcher');
  fs.mkdirSync(launcherTests, { recursive: true });
  fs.writeFileSync(path.join(launcherTests, 'fixture.test.mjs'), `
import fs from 'node:fs';
import test from 'node:test';
test('record direct Node launcher test invocation', () => {
  fs.appendFileSync(process.env.PUMAS_TEST_EVENTS, JSON.stringify({ executable: 'node-test' }) + '\\n');
});
`);
  return {
    root, context, paths,
    invoke(action, { gui = 'false', plugins = 'true', args = [], tools = true, childExit = '0' } = {}) {
      const runner = `
import { executeAction } from ${JSON.stringify(actionsUrl)};
const context = ${JSON.stringify(context)};
const paths = ${JSON.stringify(paths)};
const platformService = {
  cargoCommand: ${tools ? 'paths.cargo' : JSON.stringify(path.join(root, 'absent-cargo'))},
  corepackCommand: ${tools ? 'paths.corepack' : JSON.stringify(path.join(root, 'absent-corepack'))},
  debugBackendBinary: () => paths.debug, releaseBackendBinary: () => paths.release,
};
try {
  process.exitCode = await executeAction({ action: ${JSON.stringify(action)}, forwardedArgs: ${JSON.stringify(args)} }, { context, platformService });
} catch (error) {
  process.stderr.write(error.message + '\\n');
  process.exitCode = error.exitCode ?? 1;
}
`;
      const env = { ...process.env, PUMAS_GUI: gui, PUMAS_INFERENCE_PLUGINS: plugins,
        PUMAS_TEST_EVENTS: events, PUMAS_TEST_CHILD_EXIT: childExit };
      // This is a standalone launcher process, not a test-runner child. Do not
      // pass the parent runner's binary reporting protocol into its children.
      delete env.NODE_TEST_CONTEXT;
      const result = spawnSync(process.execPath, ['--input-type=module', '--eval', runner], {
        encoding: 'utf8', timeout: 10_000, shell: false,
        env,
      });
      assert.equal(result.error, undefined, 'the real launcher fixture process must execute');
      return result;
    },
    events() {
      return fs.existsSync(events)
        ? fs.readFileSync(events, 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line)) : [];
    },
  };
}

test('headless debug and release run require only the built backend and preserve arguments', nativeFixture, (t) => {
  const f = fixture(t);
  const args = ['--launcher-root', '/isolated root', '--rpc-port', '0', 'literal;$()'];
  for (const action of ['--run', '--run-release']) {
    const result = f.invoke(action, { args, tools: false });
    assert.equal(result.status, 0, result.stderr);
  }
  assert.deepEqual(f.events().map(({ executable, args: actual }) => [executable, actual]), [
    ['debug-backend.mjs', args], ['release-backend.mjs', args],
  ]);
  assert.equal(fs.existsSync(path.join(f.root, 'node_modules')), false);
  assert.equal(fs.existsSync(f.context.frontendDistIndex), false);
  assert.equal(fs.existsSync(f.context.electronDistMain), false);
});

test('headless missing backend and failed backend preserve explicit failures without building', nativeFixture, (t) => {
  const f = fixture(t);
  const failed = f.invoke('--run', { tools: false, childExit: '7' });
  assert.equal(failed.status, 1, failed.stderr);
  assert.match(failed.stderr, /exited with code 7/);
  fs.unlinkSync(f.paths.debug);
  fs.unlinkSync(f.paths.release);
  for (const [action, exit] of [['--run', 1], ['--run-release', 4]]) {
    const result = f.invoke(action, { tools: false });
    assert.equal(result.status, exit, result.stderr);
    assert.match(result.stderr, /missing (?:runtime backend binary|release binary)/);
  }
  assert.equal(f.events().length, 1, 'missing artifacts must not trigger build or dependency commands');
});

test('GUI and inference feature selections remain independent in both build modes', nativeFixture, (t) => {
  for (const gui of ['true', 'false']) {
    for (const plugins of ['true', 'false']) {
      for (const action of ['--build', '--build-release']) {
        const f = fixture(t, { desktop: gui === 'true' });
        const result = f.invoke(action, { gui, plugins });
        assert.equal(result.status, 0, result.stderr);
        const operations = f.events().filter(({ args }) => args[0] !== '--version');
        assert.deepEqual(operations[0].args, [
          'build', '--manifest-path', f.context.rustManifestPath, '-p', 'pumas-rpc',
          ...(action === '--build-release' ? ['--release'] : []), '--bin', 'pumas-rpc',
          ...(plugins === 'false' ? ['--no-default-features'] : []),
        ]);
        assert.deepEqual(operations.map(({ executable }) => executable), gui === 'true'
          ? ['cargo.mjs', 'corepack.mjs', 'corepack.mjs'] : ['cargo.mjs']);
        if (gui === 'true') {
          assert.deepEqual(operations[1].args, ['pnpm', '--filter', './frontend', 'run', 'build']);
          assert.equal(operations[1].plugins, plugins);
          assert.deepEqual(operations[2].args, ['pnpm', '--filter', './electron', 'run', 'build']);
        }
      }
    }
  }
});

test('headless tests select only backend crates and direct Node launcher tests', nativeFixture, (t) => {
  for (const plugins of ['true', 'false']) {
    const f = fixture(t);
    const result = f.invoke('--test', { plugins });
    assert.equal(result.status, 0, result.stderr);
    const operations = f.events().filter(({ args }) => args?.[0] !== '--version');
    assert.deepEqual(operations, [
      { executable: 'cargo.mjs', plugins, args: [
        'test', '-p', 'pumas-library', '-p', 'pumas-rpc', '--manifest-path', f.context.rustManifestPath,
        ...(plugins === 'false' ? ['--no-default-features'] : []),
      ] },
      { executable: 'node-test' },
    ]);
  }
});

test('headless install skips GUI packages; GUI-only smoke and invalid selection fail before tooling', nativeFixture, (t) => {
  const f = fixture(t);
  const installed = f.invoke('--install');
  assert.equal(installed.status, 0, installed.stderr);
  assert.deepEqual(f.events().map(({ executable, args }) => [executable, args]), [['cargo.mjs', ['--version']]]);
  const smoke = f.invoke('--release-smoke', { tools: false });
  assert.equal(smoke.status, 5, smoke.stderr);
  assert.match(smoke.stderr, /release-smoke requires PUMAS_GUI=true/);
  const invalid = f.invoke('--build', { gui: '', tools: false });
  assert.equal(invalid.status, 2, invalid.stderr);
  assert.match(invalid.stderr, /PUMAS_GUI must be true or false/);
  assert.equal(f.events().length, 1);
  const missingCargo = f.invoke('--build', { tools: false });
  assert.equal(missingCargo.status, 3, missingCargo.stderr);
  assert.match(missingCargo.stderr, /missing dependency: cargo/);
  assert.equal(f.events().length, 1);
});
