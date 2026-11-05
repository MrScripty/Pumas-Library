import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

const require = createRequire(import.meta.url);
const { createAppOriginPolicy, isMainFrameSender, openValidatedExternal } = require('../dist/window-security.js');

const entryPath = '/opt/pumas/resources/frontend/index.html';
const entryUrl = pathToFileURL(entryPath).href;

test('the bundled entry document is the only file URL that belongs to the application', () => {
  const policy = createAppOriginPolicy({ frontendEntryPath: entryPath });
  assert.equal(policy.isAppUrl(entryUrl), true);
  assert.equal(policy.isAppUrl(`${entryUrl}?view=models#top`), true);
  for (const url of [
    'file:///opt/pumas/resources/frontend/other.html',
    'file:///etc/passwd',
    'file://remote-host/opt/pumas/resources/frontend/index.html',
    'http://127.0.0.1:3000/',
    'https://huggingface.co/',
    'javascript:alert(1)',
    'not a url',
    '',
    null,
    undefined,
    42,
  ]) {
    assert.equal(policy.isAppUrl(url), false, `${String(url)} must not be an application document`);
  }
});

test('entry paths compare decoded, and case-insensitively only when asked', () => {
  const spaced = '/opt/Pumas Library/frontend/index.html';
  const policy = createAppOriginPolicy({ frontendEntryPath: spaced });
  assert.equal(policy.isAppUrl('file:///opt/Pumas%20Library/frontend/index.html'), true);
  assert.equal(policy.isAppUrl('file:///opt/pumas%20library/frontend/index.html'), false);
  const windows = createAppOriginPolicy({ frontendEntryPath: spaced, caseInsensitivePaths: true });
  assert.equal(windows.isAppUrl('file:///opt/pumas%20library/frontend/INDEX.html'), true);
  assert.equal(windows.isAppUrl('file:///opt/%E0%A4%A/frontend/index.html'), false);
});

test('the development server origin is trusted only when configured', () => {
  const policy = createAppOriginPolicy({ frontendEntryPath: entryPath, devServerOrigin: 'http://127.0.0.1:3000' });
  assert.equal(policy.isAppUrl('http://127.0.0.1:3000/'), true);
  assert.equal(policy.isAppUrl('http://127.0.0.1:3000/src/main.tsx'), true);
  assert.equal(policy.isAppUrl('http://127.0.0.1:3001/'), false);
  assert.equal(policy.isAppUrl('http://localhost:3000/'), false);
  assert.equal(policy.isAppUrl('https://127.0.0.1:3000/'), false);
});

test('main-frame sender check needs the main window, its top frame, and an application document', () => {
  const policy = createAppOriginPolicy({ frontendEntryPath: entryPath });
  const mainFrame = { url: entryUrl };
  const webContents = { mainFrame };
  const window = { isDestroyed: () => false, webContents };
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: mainFrame }, window, policy), true);
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: { url: entryUrl } }, window, policy), false);
  assert.equal(isMainFrameSender({ sender: {}, senderFrame: mainFrame }, window, policy), false);
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: null }, window, policy), false);
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: mainFrame }, null, policy), false);
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: mainFrame },
    { ...window, isDestroyed: () => true }, policy), false);
  mainFrame.url = 'https://example.test/';
  assert.equal(isMainFrameSender({ sender: webContents, senderFrame: mainFrame }, window, policy), false);
});

test('external opens are validated and refusals are logged without echoing the URL', async () => {
  const opened = [];
  const warnings = [];
  const options = {
    openExternal: async (url) => { opened.push(url); },
    log: { warn: (message) => warnings.push(message) },
  };
  assert.equal(openValidatedExternal('https://huggingface.co/org/model', options), true);
  assert.equal(openValidatedExternal('file:///etc/passwd?token=secret-value', options), false);
  assert.equal(openValidatedExternal('javascript:alert("secret-value")', options), false);
  assert.equal(openValidatedExternal(7, options), false);
  await Promise.resolve();
  assert.deepEqual(opened, ['https://huggingface.co/org/model']);
  assert.equal(warnings.length, 3);
  assert.ok(warnings.every((message) => !message.includes('secret-value')));
});
