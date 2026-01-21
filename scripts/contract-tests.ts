#!/usr/bin/env npx ts-node
/**
 * Contract Tests for Backend API
 *
 * This script validates that backend responses match the TypeScript type definitions
 * in frontend/src/types/api.d.ts. It can test both Python and Rust backends.
 *
 * Usage:
 *   npx ts-node scripts/contract-tests.ts [port]
 *
 * Default port is 9999 (matches integration tests).
 */

import * as http from 'http';

// Import types from the frontend (relative path from scripts/)
// Note: In production, you'd compile this or use ts-node with proper config
type BaseResponse = { success: boolean; error?: string };

interface StatusResponse extends BaseResponse {
  version: string;
  deps_ready: boolean;
  patched: boolean;
  menu_shortcut: boolean;
  desktop_shortcut: boolean;
  shortcut_version: string | null;
  message: string;
  comfyui_running: boolean;
  last_launch_error: string | null;
  last_launch_log: string | null;
  app_resources?: unknown;
}

interface DiskSpaceResponse extends BaseResponse {
  total: number;
  used: number;
  free: number;
  percent: number;
}

interface SystemResourcesResponse extends BaseResponse {
  resources: {
    cpu: { usage: number; temp?: number };
    gpu: { usage: number; memory: number; memory_total: number; temp?: number };
    ram: { usage: number; total: number };
    disk: { usage: number; total: number; free: number };
  };
}

interface LauncherVersionResponse extends BaseResponse {
  version: string;
  branch: string;
  isGitRepo: boolean;
}

interface SandboxInfoResponse extends BaseResponse {
  is_sandboxed: boolean;
  sandbox_type: string;
  limitations: string[];
}

interface NetworkStatusResponse extends BaseResponse {
  total_requests: number;
  successful_requests: number;
  failed_requests: number;
  circuit_breaker_rejections: number;
  retries: number;
  success_rate: number;
  circuit_states: Record<string, string>;
  is_offline: boolean;
}

interface LibraryStatusResponse extends BaseResponse {
  indexing: boolean;
  deep_scan_in_progress: boolean;
  model_count: number;
  pending_lookups?: number;
  deep_scan_progress?: {
    current: number;
    total: number;
    stage: string;
  };
}

interface LinkHealthResponse extends BaseResponse {
  status: string;
  total_links: number;
  healthy_links: number;
  broken_links: unknown[];
  orphaned_links: string[];
  warnings: string[];
  errors: string[];
}

interface FTSSearchResponse extends BaseResponse {
  models: unknown[];
  total_count: number;
  query_time_ms: number;
  query: string;
}

// =============================================================================
// RPC Client
// =============================================================================

async function rpcCall<T>(port: number, method: string, params: Record<string, unknown> = {}): Promise<T> {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      jsonrpc: '2.0',
      method,
      params,
      id: Date.now(),
    });

    const options: http.RequestOptions = {
      hostname: '127.0.0.1',
      port,
      path: '/rpc',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body),
      },
      timeout: 10000,
    };

    const req = http.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) {
            reject(new Error(json.error.message || JSON.stringify(json.error)));
          } else {
            resolve(json.result as T);
          }
        } catch (e) {
          reject(new Error(`Invalid JSON: ${data}`));
        }
      });
    });

    req.on('error', reject);
    req.on('timeout', () => {
      req.destroy();
      reject(new Error('Timeout'));
    });

    req.write(body);
    req.end();
  });
}

// =============================================================================
// Validators
// =============================================================================

type ValidationResult = { field: string; error: string };

function validate<T>(response: T, validators: Record<string, (value: unknown) => boolean>): ValidationResult[] {
  const errors: ValidationResult[] = [];
  for (const [field, validator] of Object.entries(validators)) {
    const value = (response as Record<string, unknown>)[field];
    if (!validator(value)) {
      errors.push({ field, error: `Invalid value: ${JSON.stringify(value)}` });
    }
  }
  return errors;
}

const isBoolean = (v: unknown): boolean => typeof v === 'boolean';
const isNumber = (v: unknown): boolean => typeof v === 'number';
const isString = (v: unknown): boolean => typeof v === 'string';
const isStringOrNull = (v: unknown): boolean => typeof v === 'string' || v === null;
const isArray = (v: unknown): boolean => Array.isArray(v);
const isObject = (v: unknown): boolean => typeof v === 'object' && v !== null && !Array.isArray(v);
const isOptional = (check: (v: unknown) => boolean) => (v: unknown): boolean => v === undefined || check(v);

// =============================================================================
// Contract Tests
// =============================================================================

interface TestResult {
  method: string;
  passed: boolean;
  errors: ValidationResult[];
  response?: unknown;
}

async function testGetStatus(port: number): Promise<TestResult> {
  const method = 'get_status';
  try {
    const response = await rpcCall<StatusResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      version: isString,
      deps_ready: isBoolean,
      patched: isBoolean,
      menu_shortcut: isBoolean,
      desktop_shortcut: isBoolean,
      shortcut_version: isStringOrNull,
      message: isString,
      comfyui_running: isBoolean,
      last_launch_error: isStringOrNull,
      last_launch_log: isStringOrNull,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetDiskSpace(port: number): Promise<TestResult> {
  const method = 'get_disk_space';
  try {
    const response = await rpcCall<DiskSpaceResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      total: isNumber,
      used: isNumber,
      free: isNumber,
      percent: isNumber,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetSystemResources(port: number): Promise<TestResult> {
  const method = 'get_system_resources';
  try {
    const response = await rpcCall<SystemResourcesResponse>(port, method);
    const errors: ValidationResult[] = [];

    if (!isBoolean(response.success)) {
      errors.push({ field: 'success', error: 'must be boolean' });
    }
    if (!isObject(response.resources)) {
      errors.push({ field: 'resources', error: 'must be object' });
    } else {
      if (!isObject(response.resources.cpu) || !isNumber(response.resources.cpu.usage)) {
        errors.push({ field: 'resources.cpu.usage', error: 'must be number' });
      }
      if (!isObject(response.resources.gpu) || !isNumber(response.resources.gpu.usage)) {
        errors.push({ field: 'resources.gpu.usage', error: 'must be number' });
      }
      if (!isObject(response.resources.ram) || !isNumber(response.resources.ram.usage)) {
        errors.push({ field: 'resources.ram.usage', error: 'must be number' });
      }
      if (!isObject(response.resources.disk) || !isNumber(response.resources.disk.usage)) {
        errors.push({ field: 'resources.disk.usage', error: 'must be number' });
      }
    }

    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetLauncherVersion(port: number): Promise<TestResult> {
  const method = 'get_launcher_version';
  try {
    const response = await rpcCall<LauncherVersionResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      version: isString,
      branch: isString,
      isGitRepo: isBoolean,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetSandboxInfo(port: number): Promise<TestResult> {
  const method = 'get_sandbox_info';
  try {
    const response = await rpcCall<SandboxInfoResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      is_sandboxed: isBoolean,
      sandbox_type: isString,
      limitations: isArray,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetNetworkStatus(port: number): Promise<TestResult> {
  const method = 'get_network_status';
  try {
    const response = await rpcCall<NetworkStatusResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      total_requests: isNumber,
      successful_requests: isNumber,
      failed_requests: isNumber,
      circuit_breaker_rejections: isNumber,
      retries: isNumber,
      success_rate: isNumber,
      circuit_states: isObject,
      is_offline: isBoolean,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetLibraryStatus(port: number): Promise<TestResult> {
  const method = 'get_library_status';
  try {
    const response = await rpcCall<LibraryStatusResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      indexing: isBoolean,
      deep_scan_in_progress: isBoolean,
      model_count: isNumber,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testGetLinkHealth(port: number): Promise<TestResult> {
  const method = 'get_link_health';
  try {
    const response = await rpcCall<LinkHealthResponse>(port, method);
    const errors = validate(response, {
      success: isBoolean,
      status: isString,
      total_links: isNumber,
      healthy_links: isNumber,
      broken_links: isArray,
      orphaned_links: isArray,
      warnings: isArray,
      errors: isArray,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

async function testSearchModelsFts(port: number): Promise<TestResult> {
  const method = 'search_models_fts';
  try {
    const response = await rpcCall<FTSSearchResponse>(port, method, { query: 'llama', limit: 10 });
    const errors = validate(response, {
      success: isBoolean,
      models: isArray,
      total_count: isNumber,
      query_time_ms: isNumber,
      query: isString,
    });
    return { method, passed: errors.length === 0, errors, response };
  } catch (e) {
    return { method, passed: false, errors: [{ field: 'request', error: String(e) }] };
  }
}

// =============================================================================
// Main
// =============================================================================

async function main() {
  const port = parseInt(process.argv[2] || '9999', 10);

  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║           TypeScript Contract Tests for Backend API            ║');
  console.log('╚════════════════════════════════════════════════════════════════╝\n');
  console.log(`Testing backend on port ${port}\n`);

  // Check if server is available
  try {
    await rpcCall(port, 'health_check');
    console.log('✓ Server is available\n');
  } catch (e) {
    console.error(`✗ Server not available on port ${port}`);
    console.error(`  Run the backend first: cargo run --release -- --port ${port} --launcher_root /path/to/project\n`);
    process.exit(1);
  }

  // Run all tests
  const tests = [
    testGetStatus,
    testGetDiskSpace,
    testGetSystemResources,
    testGetLauncherVersion,
    testGetSandboxInfo,
    testGetNetworkStatus,
    testGetLibraryStatus,
    testGetLinkHealth,
    testSearchModelsFts,
  ];

  const results: TestResult[] = [];

  for (const test of tests) {
    const result = await test(port);
    results.push(result);

    if (result.passed) {
      console.log(`✓ ${result.method}`);
    } else {
      console.log(`✗ ${result.method}`);
      for (const error of result.errors) {
        console.log(`  - ${error.field}: ${error.error}`);
      }
    }
  }

  // Summary
  const passed = results.filter(r => r.passed).length;
  const failed = results.filter(r => !r.passed).length;

  console.log('\n════════════════════════════════════════════════════════════════');
  console.log(`Summary: ${passed} passed, ${failed} failed`);
  console.log('════════════════════════════════════════════════════════════════');

  if (failed > 0) {
    process.exit(1);
  }
}

main().catch(console.error);
