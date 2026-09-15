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
