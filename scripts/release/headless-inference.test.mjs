import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('headless inference archive and controlled consumer admission', () => {
  const directory = fileURLToPath(new URL('.', import.meta.url));
  execFileSync(process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3'),
    ['-m', 'unittest', '-v', 'test_headless_inference'],
    { cwd: directory, stdio: 'pipe', timeout: 30_000 });
});
