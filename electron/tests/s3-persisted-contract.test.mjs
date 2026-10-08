import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require=createRequire(import.meta.url);
const {decodeS3ImportRpcResult,isS3ImportMethod}=require('../dist/s3-import-rpc.js');
const id='c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const row={operation_id:id,acquisition_id:id,phase:'using',receipt_present:true,model_binding:{model_id:'fixture/model',publication_id:id,publication_state:'pending'}};
test('receiving boundary accepts only the closed bounded informational S3 projection',()=>{
  assert.equal(isS3ImportMethod('inspect_persisted_s3_imports'),true);
  for(const result of [{status:'complete',imports:[row]},{status:'complete',imports:[]},{status:'incomplete'},{status:'unavailable'}])
    assert.deepEqual(JSON.parse(JSON.stringify(decodeS3ImportRpcResult('inspect_persisted_s3_imports',result))),result);
  for(const result of [{status:'complete',imports:Array(33).fill(row)},
    {status:'complete',imports:[{...row,credentials:{secret:'synthetic-secret'}}]},
    {status:'complete',imports:[{...row,phase:'completed'}]},
    {status:'complete',imports:[{...row,operation_id:'foreign-invalid'}]},
    {status:'complete',imports:[{...row,model_binding:{...row.model_binding,model_id:'x'.repeat(1025)}}]}])
    assert.throws(()=>decodeS3ImportRpcResult('inspect_persisted_s3_imports',result),/S3 import RPC is unavailable/);
});
