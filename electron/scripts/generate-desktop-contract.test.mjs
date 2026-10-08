import assert from 'node:assert/strict';
import test from 'node:test';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, URL } from 'node:url';
import { mkdtemp, mkdir, symlink, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { schemaType, generate } from './generate-desktop-contract.mjs';

test('bundled runtime helpers are identical across symlinked dependency workspaces', async () => {
  const workspace = await mkdtemp(join(tmpdir(), 'pumas-contract-workspace-'));
  try {
    await mkdir(join(workspace, 'node_modules'));
    await symlink(fileURLToPath(new URL('../node_modules/ajv', import.meta.url)), join(workspace, 'node_modules/ajv'), 'dir');
    const contract = {format:'pumas-desktop-contract-1',dialect:'http://json-schema.org/draft-07/schema#',
      schemas:{Text:{type:'string',minLength:1},Items:{type:'array',uniqueItems:true,items:{type:'object'}}}};
    const expected = await generate(contract);
    const relocated = await generate(contract, {runtimeRoot:workspace});
    assert.deepEqual(relocated, expected);
    assert.ok(!expected['desktop-contract.validators.js'].includes(workspace));
    assert.ok(!expected['desktop-contract.validators.js'].includes('node_modules/'));
    const validators = await import(`data:text/javascript;base64,${Buffer.from(relocated['desktop-contract.validators.js']).toString('base64')}`);
    assert.equal(validators.validateText(''), false);
    assert.equal(validators.validateItems([{a:1},{a:1}]), false);
    assert.equal(validators.validateItems([{a:1},{a:2}]), true);
  } finally { await rm(workspace, {recursive:true,force:true}); }
});

test('generated contract is independent of the invocation working directory', () => {
  const generator = new URL('./generate-desktop-contract.mjs', import.meta.url).href;
  const contract = {
    format:'pumas-desktop-contract-1', dialect:'http://json-schema.org/draft-07/schema#',
    schemas:{Text:{type:'string', minLength:1}},
  };
  const script = `import { generate } from ${JSON.stringify(generator)}; process.stdout.write(JSON.stringify(await generate(${JSON.stringify(contract)})));`;
  const outputs = ['../../', '../'].map(directory => {
    const result = spawnSync(process.execPath, ['--input-type=module', '--eval', script], {
      cwd:fileURLToPath(new URL(directory, import.meta.url)), encoding:'utf8', maxBuffer:4*1024*1024, timeout:30_000,
    });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  });
  assert.deepEqual(outputs[0], outputs[1]);
});

test('type projection preserves optional null and tagged alternatives', () => {
  assert.equal(schemaType({type:'object', required:['state'], properties:{state:{enum:['partial']}, progress:{type:['number','null']}}}), '{ "state": "partial"; "progress"?: number | null }');
});

test('type projection refuses unknown reachable schema constructs', () => {
  assert.throws(() => schemaType({type:'array',items:[{type:'string'}]}), /Unsupported/);
  assert.throws(() => schemaType({dynamicRef:'#anchor'}), /Unsupported/);
});

test('dictionary projection preserves readonly recursive references', () => {
  assert.equal(schemaType({type:'object', additionalProperties:{$ref:'#/definitions/JsonValue'}}),
    '{ readonly [key: string]: JsonValue }');
});

test('closed empty objects do not project as the non-nullish empty-object type', () => {
  assert.equal(schemaType({type:'object', additionalProperties:false}),
    'Readonly<Record<string, never>>');
});

test('standalone validator preserves UTF8 byte bounds and closed shapes', async () => {
  const files = await generate({format:'pumas-desktop-contract-1',dialect:'http://json-schema.org/draft-07/schema#',schemas:{Example:{type:'object',additionalProperties:false,required:['text'],properties:{text:{type:'string',pumasUtf8Max:4}}}}});
  const module = await import(`data:text/javascript;base64,${Buffer.from(files['desktop-contract.validators.js']).toString('base64')}`);
  assert.equal(module.validateExample({text:'éé'}), true);
  assert.equal(module.validateExample({text:'ééé'}), false);
  assert.equal(module.validateExample({text:'ok', extra:true}), false);
  assert.equal(module.validateExample({text:4}), false);
});

test('portable recovery paths reject unsafe prefixes and preserve Unicode bytes', async () => {
  const files = await generate({format:'pumas-desktop-contract-1',dialect:'http://json-schema.org/draft-07/schema#',schemas:{Path:{type:'string',pumasPortablePath:true,pumasUtf8Max:4096}}});
  const module = await import(`data:text/javascript;base64,${Buffer.from(files['desktop-contract.validators.js']).toString('base64')}`);
  assert.equal(module.validatePath('llm/example/model'), true);
  assert.equal(module.validatePath('llm/例/模型'), true);
  for (const path of ['', '/absolute','../escape','folder/CON.gguf','folder\\file','folder/file.',`folder/${'é'.repeat(128)}`]) assert.equal(module.validatePath(path), false, path);
});

test('canonical text uses Unicode White_Space rather than JavaScript trim', async () => {
  const files = await generate({format:'pumas-desktop-contract-1',dialect:'http://json-schema.org/draft-07/schema#',schemas:{Text:{type:'string',pumasCanonicalText:true}}});
  const module = await import(`data:text/javascript;base64,${Buffer.from(files['desktop-contract.validators.js']).toString('base64')}`);
  assert.equal(module.validateText('Name'), true);
  assert.equal(module.validateText('   '), false);
  assert.equal(module.validateText('\u0085Name'), false);
  assert.equal(module.validateText('\ufeffName'), true);
});

test('registered-link health refinement rejects contradictory counts and status', async () => {
  // Product invariant: every registered entry is classified exactly once.
  const files = await generate({format:'pumas-desktop-contract-1',dialect:'http://json-schema.org/draft-07/schema#',schemas:{Health:{
    type:'object', pumasLinkHealth:true,
    properties:{healthy_links:{type:'integer'},total_links:{type:'integer'},broken_links:{type:'array',items:{type:'string'}},status:{enum:['healthy','degraded']}},
    required:['healthy_links','total_links','broken_links','status'],additionalProperties:false,
  }}});
  const module = await import(`data:text/javascript;base64,${Buffer.from(files['desktop-contract.validators.js']).toString('base64')}`);
  const healthy = {healthy_links:2,total_links:2,broken_links:[],status:'healthy'};
  const degraded = {healthy_links:1,total_links:2,broken_links:['missing.gguf'],status:'degraded'};
  assert.equal(module.validateHealth(healthy),true);
  assert.equal(module.validateHealth(degraded),true);
  for (const value of [{...healthy,status:'degraded'},{...degraded,status:'healthy'},{...degraded,total_links:3},{...degraded,broken_links:null}]) {
    assert.equal(module.validateHealth(value),false);
  }
});
