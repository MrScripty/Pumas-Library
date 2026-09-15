import ctypes as c
from ctypes import wintypes as w
import tempfile
k=c.WinDLL('kernel32',use_last_error=True)
k.CreateFileW.argtypes=[w.LPCWSTR,w.DWORD,w.DWORD,c.c_void_p,w.DWORD,w.DWORD,w.HANDLE]
k.CreateFileW.restype=w.HANDLE
k.FlushFileBuffers.argtypes=[w.HANDLE]
k.CloseHandle.argtypes=[w.HANDLE]
k.LockFileEx.argtypes=[w.HANDLE,w.DWORD,w.DWORD,w.DWORD,w.DWORD,c.c_void_p]
with tempfile.TemporaryDirectory() as p:
 for access in [0x80000000,0xC0000000]:
  for flags in [0x02000000,0x82000000]:
   h=k.CreateFileW(p,access,7,None,3,flags,None)
   print('open dir',hex(access),hex(flags),h,'err',c.get_last_error(),flush=True)
   if h != c.c_void_p(-1).value:
    print('flush dir',k.FlushFileBuffers(h),'err',c.get_last_error(),flush=True)
    overlapped=c.create_string_buffer(32)
    print('lock dir',k.LockFileEx(h,3,0,1,0,overlapped),'err',c.get_last_error(),flush=True)
    k.CloseHandle(h)
k.ReOpenFile.argtypes=[w.HANDLE,w.DWORD,w.DWORD,w.DWORD]
k.ReOpenFile.restype=w.HANDLE
k.SetFileInformationByHandle.argtypes=[w.HANDLE,c.c_int,c.c_void_p,w.DWORD]
n=c.WinDLL('ntdll',use_last_error=True)
n.NtSetInformationFile.argtypes=[w.HANDLE,c.c_void_p,c.c_void_p,w.ULONG,c.c_int]
n.NtSetInformationFile.restype=c.c_long
class Rename(c.Structure):
 _fields_=[('replace',w.BYTE),('root',w.HANDLE),('length',w.DWORD),('name',w.WCHAR*1)]
import pathlib
with tempfile.TemporaryDirectory() as p:
 for share in [3,7]:
  original=k.CreateFileW(p,0x80000000,share,None,3,0x02000000,None)
  h=k.ReOpenFile(original,0xC0000000,7,0x02000000)
  print('reopen',share,h,'err',c.get_last_error(),flush=True)
  if h != c.c_void_p(-1).value:
   print('flush reopened',k.FlushFileBuffers(h),'err',c.get_last_error(),flush=True)
   k.CloseHandle(h)
  k.CloseHandle(original)
 parent=k.CreateFileW(p,0xC0000000,7,None,3,0x02000000,None)
 for native in [False,True]:
  for relative in [True,False]:
   pathlib.Path(p,'source').write_text('new')
   source=k.CreateFileW(str(pathlib.Path(p,'source')),0x80010000,7,None,3,0x02000000,None)
   name='target' if relative else str(pathlib.Path(p,'target'))
   encoded=name.encode('utf-16-le')
   buf=c.create_string_buffer(Rename.name.offset+len(encoded)+2)
   info=Rename.from_buffer(buf);info.replace=1;info.root=parent if relative else None;info.length=len(encoded)
   c.memmove(c.addressof(buf)+Rename.name.offset,encoded,len(encoded))
   if native:
    status=c.create_string_buffer(16)
    result=n.NtSetInformationFile(source,status,buf,len(buf),10)
   else: result=k.SetFileInformationByHandle(source,3,buf,len(buf))
   print('rename',native,relative,'result',hex(result&0xffffffff),'err',c.get_last_error(),flush=True)
   k.CloseHandle(source)
 k.CloseHandle(parent)
