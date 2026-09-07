import assert from 'node:assert/strict';
import test from 'node:test';
import { chooseModelImportPaths, decodeModelImportSelection } from '../dist/model-import-picker.js';

test('native selection preserves raw paths and does not retain mutable aliases', async () => {
  const paths = ['/models/ e\u0301.gguf', '/models/ e\u0301.gguf', '/models/é.gguf'];
  const selected = await chooseModelImportPaths(async () => ({ canceled: false, filePaths: paths }));
  assert.deepEqual(selected, { status: 'selected', paths });
  assert.ok(Object.isFrozen(selected.paths));
  paths.push('/later');
  assert.equal(selected.paths.length, 3);
});

test('native cancellation and rejection remain distinct and diagnostics stay private', async () => {
  assert.deepEqual(await chooseModelImportPaths(async () => ({ canceled: true, filePaths: [] })), { status: 'cancelled' });
  assert.deepEqual(await chooseModelImportPaths(async () => { throw new Error('/private/model'); }), { status: 'unavailable' });
  assert.deepEqual(await chooseModelImportPaths(async () => ({ canceled: false, filePaths: [] })), { status: 'invalid' });
});

test('closed selection decoder rejects missing fields, extras and contradictory path outcomes', () => {
  for (const value of [null, [], {success:true,paths:[]}, {status:'unknown'},
    {status:'selected'}, {status:'selected',paths:[]}, {status:'selected',paths:[42]},
    {status:'selected',paths:['']}, {status:'selected',paths:['a\0b']},
    {status:'selected',paths:['/valid'],extra:true}, {status:'cancelled',paths:[]},
    {status:'unavailable',error:'/private'},
  ]) assert.deepEqual(decodeModelImportSelection(value), {status:'invalid'});
  for (const status of ['cancelled','invalid','unavailable']) {
    assert.deepEqual(decodeModelImportSelection({status}), {status});
  }
  let accessed = false;
  assert.deepEqual(decodeModelImportSelection({ get status() { accessed = true; return 'cancelled'; } }), {status:'invalid'});
  assert.equal(accessed, false);
});
