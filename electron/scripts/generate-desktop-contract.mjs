import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath, URL } from 'node:url';
import { resolve, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import Ajv from 'ajv';
import standaloneCode from 'ajv/dist/standalone/index.js';
import { _ } from 'ajv/dist/compile/codegen/index.js';
import { build } from 'esbuild';

const root = fileURLToPath(new URL('../../', import.meta.url));
const supported = new Set(['$schema', '$id', '$ref', 'title', 'description', 'definitions', 'type', 'properties', 'required', 'additionalProperties', 'items', 'anyOf', 'oneOf', 'allOf', 'enum', 'const', 'minimum', 'maximum', 'exclusiveMinimum', 'exclusiveMaximum', 'minLength', 'maxLength', 'pattern', 'minItems', 'maxItems', 'uniqueItems', 'minProperties', 'maxProperties', 'default', 'format']);
for (const keyword of ['pumasUtf8Max', 'pumasCatalogMap', 'pumasCatalogRow', 'pumasPartialOutcome', 'pumasCatalogSearch', 'pumasMutation', 'pumasStarted', 'pumasPortablePath', 'pumasCanonicalText', 'pumasLinkHealth', 'pumasConversionSetup']) supported.add(keyword);

// Product wire refinements selected explicitly by the Rust export. AJV still
// owns all standard Draft7 behavior; none of these reinterpret schema keywords.
function addWireRefinements(ajv) {
  ajv.addKeyword({keyword:'pumasConversionSetup', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`(${data}.status === "failed") !== (${data}.error !== null)`);
  }});
  ajv.addKeyword({keyword:'pumasLinkHealth', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`!Array.isArray(${data}.broken_links) || ${data}.healthy_links + ${data}.broken_links.length !== ${data}.total_links || (${data}.status === "healthy") !== (${data}.broken_links.length === 0)`);
  }});
  ajv.addKeyword({keyword:'pumasCanonicalText', type:'string', schemaType:'boolean', code(context) {
    const expression = /^\p{White_Space}|\p{White_Space}$/u;
    const pattern = context.gen.scopeValue('pattern', {ref:expression,code:_`new RegExp(${expression.source}, "u")`});
    context.fail(_`${context.data}.length === 0 || ${pattern}.test(${context.data})`);
  }});
  ajv.addKeyword({keyword:'pumasCatalogSearch', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`!Array.isArray(${data}.models) || ${data}.total_count < ${data}.models.length || new Set(${data}.models.map(model => model.id)).size !== ${data}.models.length`);
  }});
  ajv.addKeyword({keyword:'pumasMutation', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`(${data}.success === true && ${data}.error !== undefined) || (${data}.success === false && typeof ${data}.error !== "string")`);
  }});
  ajv.addKeyword({keyword:'pumasStarted', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`${data}.selectedArtifactId !== ${data}.artifactId`);
  }});
  ajv.addKeyword({keyword:'pumasPortablePath', type:'string', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`${data}.length === 0 || ${data}.includes(String.fromCharCode(92)) || /[:*?"<>|]/.test(${data}) || Array.from(${data}).some(letter => letter.codePointAt(0) < 32 || (letter.codePointAt(0) >= 127 && letter.codePointAt(0) <= 159)) || ${data}.split("/").some(component => { const stem = component.split(".")[0].replace(/[a-z]/g, letter => letter.toUpperCase()); return component.length === 0 || component === "." || component === ".." || /[. ]$/.test(component) || encodeURIComponent(component).replace(/%[0-9A-F]{2}/g,"x").length > 255 || ["CON","PRN","AUX","NUL","CONIN$","CONOUT$"].includes(stem) || /^(COM|LPT)[1-9]$/.test(stem); })`);
  }});
  ajv.addKeyword({keyword:'pumasUtf8Max', type:'string', schemaType:'number', code(context) {
    const {data,schema} = context;
    context.fail(_`encodeURIComponent(${data}).replace(/%[0-9A-F]{2}/g, "x").length > ${schema}`);
  }});
  ajv.addKeyword({keyword:'pumasCatalogMap', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`Object.entries(${data}).some(([key, value]) => value === null || typeof value !== "object" || key !== value.id)`);
  }});
  ajv.addKeyword({keyword:'pumasCatalogRow', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`${data}.integrity?.state === "duplicate" && (!Array.isArray(${data}.integrity.otherModelIds) || ${data}.integrity.count !== ${data}.integrity.otherModelIds.length + 1 || ${data}.integrity.count < 2 || ${data}.integrity.otherModelIds.includes(${data}.id) || new Set(${data}.integrity.otherModelIds).size !== ${data}.integrity.otherModelIds.length)`);
  }});
  ajv.addKeyword({keyword:'pumasPartialOutcome', type:'object', schemaType:'boolean', code(context) {
    const {data} = context;
    context.fail(_`!(((${data}.action === "resume" || ${data}.action === "recover") && ${data}.success === true && typeof ${data}.download_id === "string" && ${data}.download_id.length > 0 && ${data}.status === "queued" && ${data}.reason_code === null && ${data}.error === null) || (${data}.action === "attach" && ${data}.success === true && typeof ${data}.download_id === "string" && ${data}.download_id.length > 0 && ["queued","downloading","pausing","cancelling"].includes(${data}.status) && ${data}.reason_code === null && ${data}.error === null) || (${data}.action === "none" && ${data}.success === false && typeof ${data}.error === "string" && ((${data}.download_id === null && ${data}.status === null && !["already_completed","already_cancelled","resume_rejected"].includes(${data}.reason_code) && ${data}.reason_code !== null) || (typeof ${data}.download_id === "string" && ${data}.download_id.length > 0 && ((${data}.status === "completed" && ${data}.reason_code === "already_completed") || (${data}.status === "cancelled" && ${data}.reason_code === "already_cancelled") || (["paused","error"].includes(${data}.status) && ${data}.reason_code === "resume_rejected"))))))`);
  }});
}

/** Project representation only; AJV owns all runtime JSON Schema semantics. */
export function schemaType(schema) {
  if (schema === true) return 'unknown';
  if (schema === false) return 'never';
  for (const key of Object.keys(schema)) {
    if (!supported.has(key)) throw new Error(`Unsupported schema construct: ${key}`);
  }
  if (schema.format !== undefined) throw new Error(`Unsupported format vocabulary: ${schema.format}`);
  for (const [keyword, value] of Object.entries(schema)) {
    if (keyword.startsWith('pumas') && keyword !== 'pumasUtf8Max' && value !== true) throw new Error(`Unsupported refinement mode: ${keyword}`);
  }
  if (schema.pumasUtf8Max !== undefined && (!Number.isSafeInteger(schema.pumasUtf8Max) || schema.pumasUtf8Max < 1)) throw new Error('Invalid UTF8 bound');
  if (schema.$ref) {
    if (!/^#\/definitions\/[A-Za-z][A-Za-z0-9_]*$/.test(schema.$ref)) throw new Error('Unsupported schema reference');
    return schema.$ref.split('/').at(-1);
  }
  if ('const' in schema) return JSON.stringify(schema.const);
  if (schema.enum) {
    if (schema.enum.some(value => value !== null && typeof value === 'object')) throw new Error('Unsupported structured enum type');
    return schema.enum.map(value => JSON.stringify(value)).join(' | ');
  }
  for (const [keyword, operator] of [['anyOf', ' | '], ['oneOf', ' | '], ['allOf', ' & ']]) {
    if (schema[keyword]) {
      if (schema.properties || schema.items) throw new Error('Unsupported combined structural type');
      return schema[keyword].map(value => `(${schemaType(value)})`).join(operator);
    }
  }
  if (Array.isArray(schema.type)) return schema.type.map(type => schemaType({...schema, type})).join(' | ');
  switch (schema.type) {
    case 'null': return 'null';
    case 'boolean': return 'boolean';
    case 'number': case 'integer': return 'number';
    case 'string': return 'string';
    case 'array':
      if (!schema.items || Array.isArray(schema.items)) throw new Error('Unsupported array shape');
      return `ReadonlyArray<${schemaType(schema.items)}>`;
    case 'object': {
      const fields = Object.entries(schema.properties ?? {}).map(([name, field]) => `${JSON.stringify(name)}${schema.required?.includes(name) ? '' : '?'}: ${schemaType(field)}`);
      if (schema.additionalProperties && typeof schema.additionalProperties === 'object') {
        if (fields.length) throw new Error('Unsupported mixed dictionary and named properties');
        // An index signature also permits recursive JSON aliases; Record's
        // mapped-type indirection makes those aliases illegal in TypeScript.
        return `{ readonly [key: string]: ${schemaType(schema.additionalProperties)} }`;
      }
      return `{ ${fields.join('; ')} }`;
    }
    default: throw new Error('Unsupported unclassified schema');
  }
}

export async function generate(contract) {
  if (contract.format !== 'pumas-desktop-contract-1' || contract.dialect !== 'http://json-schema.org/draft-07/schema#') throw new Error('Unsupported contract format/dialect');
  const ajv = new Ajv({strict: true, strictTypes: false, validateFormats: false, code: {source: true, esm: true}, allErrors: false});
  addWireRefinements(ajv);
  const types = new Map();
  const exports = {};
  for (const [name, schema] of Object.entries(contract.schemas).sort(([a], [b]) => a.localeCompare(b))) {
    if (!/^[A-Z][A-Za-z0-9_]*$/.test(name)) throw new Error('Unsupported declaration name');
    types.set(name, schemaType(schema));
    for (const [definition, value] of Object.entries(schema.definitions ?? {})) {
      if (!/^[A-Z][A-Za-z0-9_]*$/.test(definition)) throw new Error('Unsupported definition name');
      const type = schemaType(value);
      if (types.has(definition) && types.get(definition) !== type) throw new Error(`Conflicting definition ${definition}`);
      types.set(definition, type);
    }
    ajv.addSchema(schema, name);
    exports[`validate${name}`] = name;
  }
  const compiled = await build({stdin:{contents:standaloneCode(ajv, exports), resolveDir:resolve(root,'electron'), sourcefile:'desktop-contract.validators.js'}, bundle:true, platform:'browser', format:'esm', write:false, target:'es2022'});
  const hash = createHash('sha256').update(JSON.stringify(contract)).digest('hex');
  const banner = `// Generated from pumas-rpc contract.rs; SHA256 ${hash}. DO NOT EDIT.\n`;
  const names = Object.keys(contract.schemas).sort();
  const declarations = names.map(name => `export declare function validate${name}(value: unknown): boolean;`).join('\n');
  const wrappers = names.map(name => `export function decode${name}(input: unknown): DecodeOutcome<${name}> { return decode(input, validate${name}); }`).join('\n');
  const source = banner + `import { ${names.map(name => `validate${name}`).join(', ')} } from './desktop-contract.validators.js';\n` +
    [...types].sort(([a],[b])=>a.localeCompare(b)).map(([name,type])=>`export type ${name} = ${type};`).join('\n') + `\n
export type DecodeOutcome<T> = { readonly status: 'valid'; readonly value: T } | { readonly status: 'invalid' | 'unsupported' | 'unavailable'; readonly message: string };

class InvalidJsonRepresentation extends Error {}

function copyJson(value: unknown, ancestors: Set<object>, budget: { remaining: number }): unknown {
  if (--budget.remaining < 0) throw new InvalidJsonRepresentation('Oversized value');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value !== 'object' || ancestors.has(value)) throw new InvalidJsonRepresentation('Invalid JSON value');
  ancestors.add(value);
  if (Object.getOwnPropertySymbols(value).length !== 0) throw new InvalidJsonRepresentation('Invalid symbol property');
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const entries = Object.entries(descriptors).filter(([key]) => !(Array.isArray(value) && key === 'length'));
  if (Array.isArray(value) && (entries.length !== value.length || entries.some(([key]) => !/^(0|[1-9][0-9]*)$/.test(key)))) throw new InvalidJsonRepresentation('Invalid array properties');
  const result: Record<string, unknown> | unknown[] = Array.isArray(value) ? [] : Object.create(null) as Record<string, unknown>;
  for (const [key, descriptor] of entries) {
    if (!descriptor.enumerable || !('value' in descriptor)) throw new InvalidJsonRepresentation('Invalid property');
    Object.defineProperty(result, key, {value:copyJson(descriptor.value, ancestors, budget), enumerable:true, writable:false, configurable:false});
  }
  ancestors.delete(value);
  return Object.freeze(result);
}

function decode<T>(input: unknown, validate: (value: unknown) => boolean): DecodeOutcome<T> {
  try {
    const value = copyJson(input, new Set(), {remaining: 1_000_000});
    if (!validate(value)) return {status:'invalid', message:'Invalid desktop contract payload.'};
    // The generated complete validator establishes this representation.
    return {status:'valid', value:value as T};
  } catch {
    return {status:'invalid', message:'Invalid desktop contract payload.'};
  }
}
${wrappers}\n`;
  return {'desktop-contract.ts':source, 'desktop-contract.validators.js':banner+compiled.outputFiles[0].text, 'desktop-contract.validators.d.ts':banner+declarations+'\n'};
}

async function main() {
  const args = process.argv.slice(2);
  const fixtureIndex = args.indexOf('--fixtures');
  if (fixtureIndex !== -1) {
    const output = args[fixtureIndex + 1];
    if (!output) throw new Error('Missing fixture output path');
    const exported = spawnSync('cargo', ['run','--offline','--locked','-p','pumas-rpc','--no-default-features','--features','export-contract','--','--export-desktop-fixtures'], {cwd:resolve(root,'rust'), encoding:'utf8', maxBuffer:32*1024*1024});
    if (exported.status !== 0) throw new Error(`Fixture export failed: ${exported.stderr}`);
    JSON.parse(exported.stdout);
    await writeFile(output, exported.stdout, {flag:'wx', mode:0o600});
    return;
  }
  const schemaIndex = args.indexOf('--schema');
  let raw;
  if (schemaIndex !== -1) raw = await readFile(args[schemaIndex+1], 'utf8');
  else {
    const exported = spawnSync('cargo', ['run','--offline','--locked','-p','pumas-rpc','--no-default-features','--features','export-contract','--','--export-desktop-contract'], {cwd:resolve(root,'rust'), encoding:'utf8', maxBuffer:32*1024*1024});
    if (exported.status !== 0) throw new Error(`Contract export failed: ${exported.stderr}`);
    raw = exported.stdout;
  }
  const files = await generate(JSON.parse(raw));
  for (const packageName of ['electron','frontend']) {
    for (const [name, content] of Object.entries(files)) {
      const path = resolve(root, packageName, 'src/generated', name);
      if (args.includes('--check')) {
        if (await readFile(path,'utf8') !== content) throw new Error(`Stale generated contract: ${path}`);
      } else {
        await mkdir(dirname(path),{recursive:true});
        await writeFile(path,content);
      }
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
