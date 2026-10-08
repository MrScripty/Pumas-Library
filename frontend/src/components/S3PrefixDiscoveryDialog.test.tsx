import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { S3ModelImportDialog } from './S3ModelImportDialog';
import type { S3DiscoveryOutcome, S3ImportOutcome } from '../generated/desktop-contract';
const discovery=vi.hoisted(() => ({snapshot:null as S3DiscoveryOutcome|null,busy:false,error:null,start:vi.fn(),cancel:vi.fn(),reset:vi.fn(),observeAgain:vi.fn()}));
const imports=vi.hoisted(() => ({snapshot:{status:'idle'} as S3ImportOutcome,bundleProgress:null,error:null,commandBusy:false,start:vi.fn(),startAuthenticated:vi.fn(),startBundle:vi.fn(),cancel:vi.fn(),observeAgain:vi.fn()}));
vi.mock('../hooks/useS3PrefixDiscovery',()=>({useS3PrefixDiscovery:()=>discovery}));
vi.mock('../hooks/useS3ModelImport',()=>({useS3ModelImport:()=>imports}));
const objects=[{key:'models/weights.gguf',version_id:'weights-v1',size_bytes:'24',etag:'"opaque"'},
  {key:'models/config.json',version_id:'config-v1',size_bytes:'2',etag:'"opaque"'}];
describe('explicit prefix selection in the existing import dialog',()=>{
  beforeEach(()=>{vi.clearAllMocks();imports.snapshot={status:'idle'};discovery.busy=false;discovery.snapshot={status:'complete',operation_id:'fixture',pages:2,objects};});
  it('keeps discovery wording accurate after successful registration',()=>{
    imports.snapshot={status:'finished',operation_id:'completed-operation',result:{status:'completed',model_id:'fixture/registered'}};
    render(<S3ModelImportDialog onClose={vi.fn()}/>);
    expect(screen.getByText('Model registered: fixture/registered')).toBeInTheDocument();
    expect(screen.getByText(/Complete discovery: 2 objects across 2 pages\. Discovery does not import files\./)).toBeInTheDocument();
    expect(screen.queryByText(/Nothing has been imported/)).not.toBeInTheDocument();
    expect(imports.start).not.toHaveBeenCalled();expect(imports.startBundle).not.toHaveBeenCalled();
  });
  it('does not import discovery and requires explicit output paths and trusted hashes',()=>{
    render(<S3ModelImportDialog onClose={vi.fn()}/>);
    expect(imports.start).not.toHaveBeenCalled();expect(imports.startBundle).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button',{name:'Use models/weights.gguf as primary'}));
    expect(screen.getByLabelText('Exact object key')).toHaveValue('models/weights.gguf');
    expect(screen.getByLabelText('Immutable VersionId')).toHaveValue('weights-v1');
    expect(screen.getByLabelText('Library GGUF filename')).toHaveValue('');expect(screen.getByLabelText('Expected SHA-256')).toHaveValue('');
    fireEvent.click(screen.getByRole('button',{name:'Add models/config.json as auxiliary'}));
    expect(screen.getByLabelText('Auxiliary 1 logical output path')).toHaveValue('');expect(screen.getByLabelText('Auxiliary 1 expected SHA-256')).toHaveValue('');
    for(const [label,value] of [['Library GGUF filename','weights.gguf'],['Expected SHA-256','a'.repeat(64)],['Auxiliary 1 logical output path','config/data.json'],['Auxiliary 1 expected SHA-256','b'.repeat(64)]] as const)
      fireEvent.change(screen.getByLabelText(label),{target:{value}});
    const form=screen.getByRole('button',{name:'Import pinned object'}).closest('form');
    expect(form).not.toBeNull();if(form) fireEvent.submit(form);
    expect(imports.startBundle).toHaveBeenCalledWith(expect.objectContaining({key:'models/weights.gguf',version_id:'weights-v1',filename:'weights.gguf',sha256:'a'.repeat(64)}),
      [{key:'models/config.json',version_id:'config-v1',logical_path:'config/data.json',sha256:'b'.repeat(64)}]);
  });
  it('revokes discovery-derived pins when the source changes',()=>{
    render(<S3ModelImportDialog onClose={vi.fn()}/>);
    fireEvent.click(screen.getByRole('button',{name:'Use models/weights.gguf as primary'}));
    fireEvent.click(screen.getByRole('button',{name:'Add models/config.json as auxiliary'}));
    fireEvent.change(screen.getByLabelText('Bucket'),{target:{value:'different-bucket'}});
    expect(screen.getByLabelText('Exact object key')).toHaveValue('');expect(screen.queryByLabelText('Auxiliary 1 logical output path')).not.toBeInTheDocument();
    expect(discovery.reset).toHaveBeenCalled();
  });
  it('clears one-use discovery credentials before waiting and closing cancels only discovery',()=>{
    const close=vi.fn();render(<S3ModelImportDialog onClose={close}/>);
    fireEvent.click(screen.getByLabelText('Use one-use credentials'));
    fireEvent.change(screen.getByLabelText('Access key ID'),{target:{value:'synthetic-key'}});
    const secret=screen.getByLabelText('Secret access key');fireEvent.change(secret,{target:{value:'synthetic-secret'}});
    fireEvent.click(screen.getByRole('button',{name:'Discover prefix'}));
    expect(secret).toHaveValue('');expect(discovery.start).toHaveBeenCalledWith(expect.anything(),'models/',{access_key_id:'synthetic-key',secret_access_key:'synthetic-secret',session_token:null});
    fireEvent.click(screen.getByRole('button',{name:'Close'}));expect(discovery.reset).toHaveBeenCalled();expect(imports.cancel).not.toHaveBeenCalled();expect(close).toHaveBeenCalledOnce();
  });
  it('offers no selectable rows for incomplete, deadline, unavailable or empty results',()=>{
    discovery.snapshot={status:'incomplete',operation_id:'fixture'};const view=render(<S3ModelImportDialog onClose={vi.fn()}/>);
    expect(screen.queryByRole('button',{name:/as primary/})).not.toBeInTheDocument();expect(screen.getByText(/no partial selection/)).toBeInTheDocument();
    discovery.snapshot={status:'complete',operation_id:'fixture',pages:1,objects:[]};view.rerender(<S3ModelImportDialog onClose={vi.fn()}/>);expect(screen.getByText(/0 objects/)).toBeInTheDocument();
    for(const value of [{status:'deadline',operation_id:'fixture'},{status:'unavailable'}] as S3DiscoveryOutcome[]) {
      discovery.snapshot=value;view.rerender(<S3ModelImportDialog onClose={vi.fn()}/>);expect(screen.queryByRole('button',{name:/as primary/})).not.toBeInTheDocument();
    }
  });
});
