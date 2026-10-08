# Shared static-reader helpers. Loading the audited tool library is permitted;
# target executables are parsed as bytes and never loaded through Reflection.
function Get-ResearchCanonicalPath([string]$Path) {
    if (-not ('RewriteResearchPaths' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class RewriteResearchPaths {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern SafeFileHandle CreateFileW(string path, uint access, uint share,
        IntPtr security, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern uint GetFinalPathNameByHandleW(SafeFileHandle file, StringBuilder path, uint size, uint flags);
    public static string Resolve(string path) {
        // Metadata-only, shared existing handle; backup semantics permits directories.
        using (var file = CreateFileW(path, 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {
            if (file.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
            var buffer = new StringBuilder(512);
            uint size = GetFinalPathNameByHandleW(file, buffer, (uint)buffer.Capacity, 0);
            if (size == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
            if (size >= buffer.Capacity) {
                if (size > 32768) throw new InvalidOperationException("Canonical path exceeds research limit");
                buffer = new StringBuilder((int)size + 1);
                size = GetFinalPathNameByHandleW(file, buffer, (uint)buffer.Capacity, 0);
                if (size == 0 || size >= buffer.Capacity) throw new Win32Exception(Marshal.GetLastWin32Error());
            }
            string result = buffer.ToString();
            if (result.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("UNC research paths are not qualified");
            if (result.StartsWith(@"\\?\")) result = result.Substring(4);
            return result.TrimEnd('\\');
        }
    }
}
'@
    }
    return [RewriteResearchPaths]::Resolve([IO.Path]::GetFullPath($Path))
}

function Initialize-ManagedReader([string]$ToolDirectory) {
    $package = Join-Path $ToolDirectory 'icsharpcode.decompiler.9.1.0.7988.nupkg'
    $dll = Join-Path $ToolDirectory 'package/lib/netstandard2.0/ICSharpCode.Decompiler.dll'
    if ((Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash -ne
        'CCF2E02CD3B8702AED37D051F71E97F96D0C36443BD67003228A4E0B55418404') {
        throw 'Unaudited ILSpy package identity'
    }
    # Compare the installed library with the DLL inside the pinned package too.
    $archive = [IO.Compression.ZipFile]::OpenRead($package)
    try {
        $entry = $archive.GetEntry('lib/netstandard2.0/ICSharpCode.Decompiler.dll')
        $stream = $entry.Open()
        try { $dllHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) }
        finally { $stream.Dispose() }
    } finally { $archive.Dispose() }
    if ((Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash -ne $dllHash) {
        throw 'ILSpy library differs from pinned package'
    }
    $assembly = [Reflection.Assembly]::LoadFrom($dll)
    if ($assembly.GetName().Version.ToString() -ne '9.1.0.7988') { throw 'Unexpected reader version' }
    return [ordered]@{version='9.1.0.7988';package_sha256='ccf2e02cd3b8702aed37d051f71e97f96d0c36443bd67003228a4e0b55418404';dll_sha256=$dllHash.ToLowerInvariant();host_runtime=[Environment]::Version.ToString()}
}

function Assert-PrivateResearchDirectory([string]$Directory, [string]$SourceRoot) {
    $item = Get-Item -LiteralPath $Directory
    if (!$item.PSIsContainer) { throw 'Output directory must exist' }
    for ($ancestor = $item; $null -ne $ancestor; $ancestor = $ancestor.Parent) {
        if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Output ancestors may not be links' }
        if (Test-Path -LiteralPath (Join-Path $ancestor.FullName '.git')) { throw 'Research output inside Git checkout' }
    }
    $full=Get-ResearchCanonicalPath $item.FullName
    $privateRoot=Get-ResearchCanonicalPath (Join-Path $env:LOCALAPPDATA 'KH2FinalRewrite/research')
    if (!$full.Equals($privateRoot,[StringComparison]::OrdinalIgnoreCase) -and
        !$full.StartsWith($privateRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
        throw 'Managed research output must be under the private KH2FinalRewrite research root'
    }
    if ($SourceRoot) {
        $root=Get-ResearchCanonicalPath $SourceRoot
        if ($full.Equals($root, [StringComparison]::OrdinalIgnoreCase) -or
            $full.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Research output inside installed source'
        }
    }
    return $full
}

function Write-NewResearchFile([string]$Path, [byte[]]$Bytes) {
    $file = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $file.Write($Bytes); $file.Flush($true) } finally { $file.Dispose() }
}

function New-StaticDecompiler([byte[]]$Bytes, [string]$PrivateBasePath) {
    $memory = [IO.MemoryStream]::new($Bytes, $false)
    $module = $null
    try {
        $module = [ICSharpCode.Decompiler.Metadata.PEFile]::new($PrivateBasePath,$memory,
            [Reflection.PortableExecutable.PEStreamOptions]::PrefetchEntireImage,
            [Reflection.Metadata.MetadataReaderOptions]::Default)
        $settings = [ICSharpCode.Decompiler.DecompilerSettings]::new()
        $settings.UseDebugSymbols = $false
        $settings.ThrowOnAssemblyResolveErrors = $false
        # Resolver's base is private, never the Steam directory. Missing symbols
        # stay unresolved; no target sibling assemblies are silently admitted.
        $resolver = [ICSharpCode.Decompiler.Metadata.UniversalAssemblyResolver]::new(
            $PrivateBasePath,$false,
            [ICSharpCode.Decompiler.Metadata.DotNetCorePathFinderExtensions]::DetectTargetFrameworkId($module),$null,
            [Reflection.PortableExecutable.PEStreamOptions]::PrefetchMetadata,
            [Reflection.Metadata.MetadataReaderOptions]::Default)
        $decompiler = [ICSharpCode.Decompiler.CSharp.CSharpDecompiler]::new($module,$resolver,$settings)
        return [pscustomobject]@{module=$module;memory=$memory;decompiler=$decompiler}
    } catch { if ($null -ne $module) {$module.Dispose()}; $memory.Dispose(); throw }
}
