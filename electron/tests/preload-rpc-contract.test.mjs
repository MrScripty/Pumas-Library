import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { RPC_METHOD_REGISTRY } from '../dist/rpc-method-registry.js';

const DEFERRED_UNREGISTERED_PRELOAD_METHODS = [];

const PRELOAD_SOURCE = readFileSync(new URL('../src/preload.ts', import.meta.url), 'utf8');

function preloadRpcMethodNames() {
  return [
    ...new Set(
      [...PRELOAD_SOURCE.matchAll(/apiCall\('([^']+)'/g)]
        .map((match) => match[1])
        .filter((methodName) => methodName !== undefined)
    ),
  ].sort();
}

test('preload apiCall methods are registered or tracked deferred drift', () => {
  const registeredMethods = new Set(RPC_METHOD_REGISTRY.methods);
  const unregisteredMethods = preloadRpcMethodNames()
    .filter((methodName) => !registeredMethods.has(methodName))
    .sort();

  assert.deepEqual(unregisteredMethods, DEFERRED_UNREGISTERED_PRELOAD_METHODS);
});

test('deferred preload drift exceptions still describe live drift', () => {
  const registeredMethods = new Set(RPC_METHOD_REGISTRY.methods);
  const preloadMethods = new Set(preloadRpcMethodNames());

  for (const methodName of DEFERRED_UNREGISTERED_PRELOAD_METHODS) {
    assert.ok(preloadMethods.has(methodName), `${methodName} is no longer forwarded by preload`);
    assert.ok(!registeredMethods.has(methodName), `${methodName} is now registered`);
  }
});
