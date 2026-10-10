// Qualification driver only. Import and execute the pinned Chrema consumer;
// observe real subprocesses/transports without replacing any peer or result.
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import http from 'node:http';
import {syncBuiltinESMExports} from 'node:module';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createInterface} from 'node:readline';

const [sourceRoot, binary, binarySha256, libraryRoot, descriptionFile, mode] = process.argv.slice(2);
assert(['normal-release', 'caller-cancel', 'owner-revocation'].includes(mode));
const expected = JSON.parse(await readFile(descriptionFile, 'utf8'));
const children = [], requests = [];
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const realSpawn = childProcess.spawn;
childProcess.spawn = function (...args) {
  const child = Reflect.apply(realSpawn, this, args);
  const record = {binary: args[0], arguments: args[1], pid: child.pid, closed: false};
  const chunks = []; let bytes = 0;
  child.stdout.on('data', chunk => {bytes += chunk.length; if (bytes <= 65536) chunks.push(chunk);});
  child.once('close', (code, signal) => {
    Object.assign(record, {closed: true, exit_code: code, signal, stdout_bytes: bytes});
    const output = Buffer.concat(chunks); record.stdout_sha256 = hash(output);
    if (bytes <= 65536 && code === 0) {
      try {record.description = JSON.parse(output.toString('utf8'));}
      catch (error) {record.observation_error = error.message;}
    }
  });
  children.push(record);
  return child;
};
const realRequest = http.request;
http.request = function (...args) {
  const request = Reflect.apply(realRequest, this, args);
  const record = {method: request.method, path: request.path,
    instance_generation: request.getHeader('Pumas-Instance-Generation'),
    service_generation: request.getHeader('Pumas-Service-Generation'), closed: false};
  const realEnd = request.end;
  request.end = function (...endArgs) {
    if (Buffer.isBuffer(endArgs[0]) || typeof endArgs[0] === 'string') {
      try {record.request_body = JSON.parse(endArgs[0].toString());}
      catch (error) {record.observation_error = error.message;}
    }
    return Reflect.apply(realEnd, this, endArgs);
  };
  request.once('close', () => {record.closed = true;});
  request.once('response', response => {
    record.status = response.statusCode;
    record.content_type = response.headers['content-type'];
    const chunks = []; let bytes = 0;
    response.on('data', chunk => {bytes += chunk.length; if (bytes <= 65536) chunks.push(chunk);});
    response.once('close', () => {
      record.response_bytes = bytes;
      record.response_body = Buffer.concat(chunks).toString('utf8');
      record.response_closed = true;
    });
  });
  requests.push(record);
  return request;
};
syncBuiltinESMExports();

const {openPumasLocalConsumer} = await import(pathToFileURL(join(sourceRoot, 'agent-platform-pumas-local-session.mjs')));
const lifetime = new AbortController();
const grants = []; let allowCatalog = true;
const observation = {mode, node_version: process.version, caller_pid: process.pid,
  observer_children: children, actual_http_requests: requests,
  observation_instrumentation: 'Wrappers log real spawn/request/close/data; original functions, arguments, bytes and outcomes unchanged.'};
let consumer;
try {
  consumer = await openPumasLocalConsumer({binary, binarySha256, libraryRoot,
    environment: {...process.env}, signal: lifetime.signal,
    authorizeCatalog: async ({signal}) => {grants.push({method: 'get_models', allowed: allowCatalog, signal_supplied: signal !== undefined}); return allowCatalog;},
    authorizeLookup: async ({modelId, signal}) => {grants.push({method: 'lookup_model', model_id: modelId, allowed: true, signal_supplied: signal !== undefined}); return true;},
  });
  assert.equal(consumer.ownership, 'borrowed');
  assert.equal(consumer.sourceCommit, 'fe3c9f04c243f436a393ae47e16f616244a85cee');
  assert.equal(consumer.endpoint, expected.endpoint);
  assert.deepEqual(consumer.admissionFence, {
    instanceGeneration: expected.instance.generation, serviceGeneration: expected.service_generation,
  });
  assert.equal(consumer.retainedOwner.assertCurrent(), true);
  observation.catalog = await consumer.getModels({signal: lifetime.signal});
  assert.deepEqual(observation.catalog, {modelIds: []});
  observation.lookup = await consumer.lookupModel('unknown/fixture/absent', {signal: lifetime.signal});
  assert.equal(observation.lookup.contract_version, 1);
  assert.equal(observation.lookup.requested_model_id, 'unknown/fixture/absent');
  assert.equal(observation.lookup.resolution.status, 'missing');
  assert.equal(grants.length, 4); // Fresh host authorization before dispatch and disclosure.
  const sentBeforeInvalid = requests.length;
  await assert.rejects(consumer.lookupModel('unknown//invalid'), TypeError);
  assert.equal(requests.length, sentBeforeInvalid);
  assert.equal(grants.length, 4);
  allowCatalog = false;
  await assert.rejects(consumer.getModels(), {code: 'PUMAS_CATALOG_AUTHORIZATION_DENIED'});
  assert.equal(requests.length, sentBeforeInvalid);
  allowCatalog = true;
  observation.catalog_after_denial = await consumer.getModels({signal: lifetime.signal});
  assert.deepEqual(observation.catalog_after_denial, {modelIds: []});
  assert.equal(grants.length, 7);
  assert.deepEqual(grants.map(grant => grant.method), [
    'get_models', 'get_models', 'lookup_model', 'lookup_model', 'get_models', 'get_models', 'get_models',
  ]);
  observation.host_grants = grants;
  observation.ownership = consumer.ownership;
  observation.endpoint = consumer.endpoint;
  observation.admission_fence = consumer.admissionFence;
  assert.equal(children.length, 2);
  for (const child of children) {
    assert.equal(child.binary, binary);
    assert.deepEqual(child.arguments, ['--describe-local-http', '--launcher-root', libraryRoot]);
    assert.equal(child.closed, true); assert.equal(child.exit_code, 0); assert.equal(child.signal, null);
    assert.equal(child.observation_error, undefined);
    assert.deepEqual(child.description, expected);
  }
  assert.equal(requests.length, 4); // One native retention stream, three native RPCs.
  assert.equal(requests[0].path, '/.well-known/pumas/retention');
  assert.equal(requests[0].status, 200);
  console.log(JSON.stringify({phase: 'ready', ownership: consumer.ownership, admission_fence: consumer.admissionFence}));

  if (mode === 'owner-revocation') {
    if (!consumer.signal.aborted) await new Promise(resolve => consumer.signal.addEventListener('abort', resolve, {once: true}));
    observation.native_owner_revocation_observed = true;
  } else {
    const input = createInterface({input: process.stdin, crlfDelay: Infinity});
    const [line] = await input[Symbol.asyncIterator]().next().then(value => [value.value]);
    input.close(); assert.equal(line, 'finish');
    assert.equal(consumer.retainedOwner.assertCurrent(), true);
    if (mode === 'caller-cancel') lifetime.abort(new Error('Qualification caller ended.'));
  }
  const released = consumer.release();
  assert.equal(released, consumer.release());
  await released;
  assert.equal(consumer.signal.aborted, true);
  assert.equal(consumer.retainedOwner.signal.aborted, true);
  for (const request of requests) {
    assert.equal(request.observation_error, undefined);
    assert.equal(request.closed, true); assert.equal(request.response_closed, true); assert.equal(request.status, 200);
    assert.equal(request.instance_generation, expected.instance.generation);
    assert.equal(request.service_generation, expected.service_generation);
  }
  const retention = requests[0].response_body;
  assert.match(retention, /event: pumas-owner-retention/);
  assert.match(retention, /"state":"retained"/);
  if (mode === 'owner-revocation') assert.match(retention, /"state":"revoked"/);
  for (const request of requests.slice(1)) {
    const reply = JSON.parse(request.response_body);
    assert.equal(reply.id, request.request_body.id); assert.equal(reply.jsonrpc, '2.0');
    assert.equal(Object.hasOwn(reply, 'result'), true); assert.equal(Object.hasOwn(reply, 'error'), false);
  }
  const sentAfterClose = requests.length;
  await assert.rejects(consumer.getModels(), {code: 'PUMAS_CATALOG_OWNER_UNAVAILABLE'});
  await assert.rejects(consumer.lookupModel('unknown/fixture/absent'), {code: 'PUMAS_CATALOG_OWNER_UNAVAILABLE'});
  assert.equal(requests.length, sentAfterClose);
  assert.equal(children.length, 2);
  observation.status = 'passed';
  observation.caller_signal_aborted = lifetime.signal.aborted;
  observation.consumer_signal_aborted = consumer.signal.aborted;
  observation.retention_signal_aborted = consumer.retainedOwner.signal.aborted;
  observation.release_settled = true;
  observation.closed_consumer_refused_without_transport_or_observer = true;
} catch (error) {
  observation.status = 'failed'; observation.error = {name: error.name, code: error.code, message: error.message};
  if (consumer) {
    try {await consumer.release(); observation.release_settled = true;}
    catch (cleanup) {observation.cleanup_error = {name: cleanup.name, message: cleanup.message};}
  }
  process.exitCode = 1;
}
console.log(JSON.stringify({phase: 'finished', observation}));
