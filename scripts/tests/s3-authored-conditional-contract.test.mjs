import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

const source = {
  operation_id:'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa', endpoint:'https://localhost:443',
  region:'fixture',bucket:'fixture',addressing:'path',family:'fixture',official_name:'Authored fixture',
  read_mode:'conditional',primary_logical_path:'weights.gguf',files:[
    {key:'objects/weights',logical_path:'weights.gguf',sha256:'0'.repeat(64),expected_etag:'"selected"',expected_size:'24'},
    {key:'objects/notes',logical_path:'notes.txt',sha256:'0'.repeat(64),expected_etag:'""',expected_size:'0'},
  ],
};
const credentials={access_key_id:'fixture-access',secret_access_key:'fixture-secret'};
for (const consumer of ['electron','frontend']) {
  const bytes = await readFile(new URL(`../../${consumer}/src/generated/desktop-contract.validators.js`,import.meta.url));
  const wire = await import(`data:text/javascript;base64,${bytes.toString('base64')}`);
  test(`${consumer}: closed uniformly authored sets and nested authenticated definitions`,()=>{
    assert.equal(wire.validateS3BundleImportParams(source),true);
    assert.equal(wire.validateS3AuthenticatedBundleImportParams({source,credentials}),true);
    const pinned=structuredClone(source);
    delete pinned.read_mode;
    pinned.files=pinned.files.map(({expected_etag,expected_size,...file})=>({...file,version_id:'v1'}));
    assert.equal(wire.validateS3BundleImportParams(pinned),true);
    assert.equal(wire.validateS3PinnedFileParams(pinned.files[0]),true);
    const faults=[];
    for (const mode of [undefined,'version_id','unknown',null]) {
      const row=structuredClone(source);
      if(mode===undefined) delete row.read_mode; else row.read_mode=mode;
      faults.push(row);
    }
    for(const version_id of ['',null,'null','v1']) {
      const row=structuredClone(source);row.files[0].version_id=version_id;faults.push(row);
    }
    for(const count of [0,1,33]) {const row=structuredClone(source);row.files=Array.from({length:count},()=>({...source.files[0]}));faults.push(row);}
    const mixed=structuredClone(source);mixed.files[1]=pinned.files[1];faults.push(mixed);
    const long=structuredClone(source);long.files[1].logical_path='a/'.repeat(520)+'data.json';faults.push(long);
    for(const row of faults) {
      assert.equal(wire.validateS3BundleImportParams(row),false);
      assert.equal(wire.validateS3AuthenticatedBundleImportParams({source:row,credentials}),false);
    }
    const prefix={...source,prefix:'',timeout_ms:1000};
    for(const key of ['files','primary_logical_path','family','official_name']) delete prefix[key];
    assert.equal(wire.validateS3DiscoveryParams(prefix),false);
    assert.equal(wire.validateS3AuthenticatedDiscoveryParams({source:prefix,credentials}),false);
  });
  test(`${consumer}: exact SDK size strings and native opaque ETags`,()=>{
    for(const expected_size of ['0','1','9007199254740993','9223372036854775807']) {
      for(const expected_etag of ['""','"opaque-é-😀"','"selected"']) {
        const row=structuredClone(source);Object.assign(row.files[1],{expected_size,expected_etag});
        assert.equal(wire.validateS3BundleImportParams(row),true);
        assert.equal(wire.validateS3AuthenticatedBundleImportParams({source:row,credentials}),true);
      }
    }
    for(const [field,values] of Object.entries({
      expected_size:[null,0,1,'','00','01','-1','+1','1e2','1 ','1\n','9223372036854775808','18446744073709551616'],
      expected_etag:[null,'selected','W/"selected"','"a"b"','"a\nb"','"a\u007fb"'],sha256:[null,'','x'.repeat(64)],
    })) for(const value of values) {
      const row=structuredClone(source);row.files[1][field]=value;
      assert.equal(wire.validateS3BundleImportParams(row),false,`${field}: ${value}`);
      assert.equal(wire.validateS3AuthenticatedBundleImportParams({source:row,credentials}),false);
    }
    for(const field of ['expected_size','expected_etag','sha256']) {
      const row=structuredClone(source);delete row.files[1][field];assert.equal(wire.validateS3BundleImportParams(row),false);
    }
  });
}
