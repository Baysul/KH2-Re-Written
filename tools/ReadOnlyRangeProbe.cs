using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

// Windows x64 read/query only. Callers bind source, process and selected spans.
public static class ReadOnlyRangeProbe {
    [StructLayout(LayoutKind.Explicit, Size=48)]
    public struct Region {
        [FieldOffset(0)] public ulong BaseAddress;
        [FieldOffset(8)] public ulong AllocationBase;
        [FieldOffset(16)] public uint AllocationProtect;
        [FieldOffset(24)] public ulong RegionSize;
        [FieldOffset(32)] public uint State;
        [FieldOffset(36)] public uint Protect;
        [FieldOffset(40)] public uint Type;
    }
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern SafeProcessHandle OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern nuint VirtualQueryEx(SafeProcessHandle process, IntPtr address, out Region region, nuint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool ReadProcessMemory(SafeProcessHandle process, IntPtr address, byte[] buffer, nuint size, out nuint read);
    static SafeProcessHandle Open(int pid) {
        if (!Environment.Is64BitProcess || pid<=0) throw new ArgumentOutOfRangeException(nameof(pid));
        var handle=OpenProcess(0x410, false, pid); // VM_READ | QUERY_INFORMATION
        if (handle.IsInvalid) {int error=Marshal.GetLastWin32Error();handle.Dispose();throw new Win32Exception(error);}
        return handle;
    }
    static void Bound(long address, int count) {
        if (address<=0 || count<1 || count>4096 || address>long.MaxValue-count)
            throw new ArgumentOutOfRangeException(nameof(address));
    }
    public static Region Query(int pid, long address) {
        Bound(address,1);
        using var handle=Open(pid);
        nuint returned=VirtualQueryEx(handle,new IntPtr(address),out Region region,48);
        if (returned==0) throw new Win32Exception(Marshal.GetLastWin32Error());
        if (returned!=48) throw new InvalidOperationException("Unexpected Windows x64 memory-query ABI");
        return region;
    }
    public static byte[] Read(int pid, long address, int count) {
        Bound(address,count);
        using var handle=Open(pid);
        var bytes=new byte[count];
        if (!ReadProcessMemory(handle,new IntPtr(address),bytes,(nuint)count,out nuint read))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        if (read!=(nuint)count) throw new InvalidOperationException("Partial bounded memory read");
        return bytes;
    }
    public static void RequireImageRegion(Region region, long address, int count, long moduleBase, bool executable) {
        Bound(address,count);
        uint basic=region.Protect & 0xff;
        bool readable=basic==0x02 || basic==0x04 || basic==0x08 || basic==0x20 || basic==0x40 || basic==0x80;
        bool code=basic==0x20 || basic==0x40 || basic==0x80;
        if (moduleBase<=0 || region.AllocationBase!=(ulong)moduleBase || region.Type!=0x1000000 ||
            region.State!=0x1000 || !readable || (executable && !code) || (region.Protect & 0x100)!=0 ||
            region.RegionSize==0 || region.BaseAddress>ulong.MaxValue-region.RegionSize ||
            (ulong)address<region.BaseAddress || (ulong)(address+count)>region.BaseAddress+region.RegionSize)
            throw new InvalidOperationException("Selected span lacks a committed readable image region");
    }
}
