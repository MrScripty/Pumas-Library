import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('headless archive byte pins and controlled packaging refusals', () => {
  const directory = fileURLToPath(new URL('.', import.meta.url));
  execFileSync(process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3'),
    ['-m', 'unittest', '-v', 'test_headless_archive'],
    { cwd: directory, stdio: 'pipe', timeout: 30_000 });
});
