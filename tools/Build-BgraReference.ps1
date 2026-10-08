# Hash-pinned public scanline branches in an owned bounded private adapter.
param(
    [Parameter(Mandatory)][string]$PublicSourceDirectory,
    [Parameter(Mandatory)][string]$BuildDirectory,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$CompilerPath
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$source=Assert-PrivateResearchDirectory $PublicSourceDirectory $InstallRoot
$output=Assert-PrivateResearchDirectory $BuildDirectory $InstallRoot
$path=Join-Path $source 'DirectXTexConvert.cpp'
if((Get-FileHash $path).Hash -ne 'BD7E8BAD4F6D7A58C9E61740F4638ADF3D37DADE9EC060AB133820F707F3427A'){
    throw 'DirectXTex scanline input differs from pin 1acf4ebe6330e06b7eb52e8db63d4f824ad77427'
}
$methods=Join-Path $output 'BgraReferenceBranches.h'
$exe=Join-Path $output 'BgraReference.exe'
$obj=Join-Path $output 'BgraReference.obj'
foreach($destination in @($methods,$exe,$obj)){if(Test-Path $destination){throw 'BGRA reference build output exists'}}
$text=Get-Content $path -Raw
function Get-PublicRange([string]$Start,[string]$End,[int]$From=0){
    $first=$text.IndexOf($Start,$From,[StringComparison]::Ordinal)
    if($first -lt 0){throw 'Public branch start absent'}
    $last=$text.IndexOf($End,$first,[StringComparison]::Ordinal)
    if($last -le $first){throw 'Public branch end absent'}
    $text.Substring($first,$last-$first)
}
$load=Get-PublicRange '    case DXGI_FORMAT_B8G8R8A8_UNORM:' '    case DXGI_FORMAT_B8G8R8X8_UNORM:' ($text.IndexOf('DirectX::Internal::LoadScanline('))
$store=Get-PublicRange '    case DXGI_FORMAT_R8G8B8A8_UNORM:' '    case DXGI_FORMAT_R8G8B8A8_UINT:' ($text.IndexOf('DirectX::Internal::StoreScanline('))
$bias=Get-PublicRange '    constexpr float   g_8BitBias ' '    const XMVECTORF32' # scalar declaration
$vectorStart=$text.IndexOf('    const XMVECTORF32 g_8BitBiasV')
if($vectorStart -lt 0){throw 'Public byte bias absent'}
$vectorEnd=$text.IndexOf(';',$vectorStart)
$bias+=$text.Substring($vectorStart,$vectorEnd-$vectorStart+1)
# Entire case statements and bias declarations are unchanged. Only switch/input
# scaffolding is owned; no general DirectXTex importer or SRGB conversion runs.
$header='using namespace DirectX;using namespace DirectX::PackedVector;'+$bias+@'
static bool PublicLoadBgra(XMVECTOR* pDestination,size_t count,const void* pSource,size_t size) {
    XMVECTOR* __restrict dPtr=pDestination;const XMVECTOR* ePtr=pDestination+count;
    switch(DXGI_FORMAT_B8G8R8A8_UNORM) {
'@+$load+@'
    default:return false;}
}
static bool PublicStoreRgba(void* pDestination,size_t size,const XMVECTOR* pSource,size_t count) {
    const XMVECTOR* __restrict sPtr=pSource;const XMVECTOR* ePtr=pSource+count;
    switch(DXGI_FORMAT_R8G8B8A8_UNORM) {
'@+$store+@'
    default:return false;}
}
'@
Write-NewResearchFile $methods ([Text.Encoding]::UTF8.GetBytes($header))
$extracts=@(foreach($part in @($load,$store,$bias)){@{length=$part.Length;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($part))).ToLowerInvariant()}})
Write-NewResearchFile (Join-Path $output 'public-branches.json') ([Text.Encoding]::UTF8.GetBytes((@{pin='1acf4ebe6330e06b7eb52e8db63d4f824ad77427';source_sha256=(Get-FileHash $path).Hash.ToLowerInvariant();unchanged_extracts=$extracts}|ConvertTo-Json -Depth 6)))
& $CompilerPath /nologo /std:c++17 /EHsc /W4 /WX /O2 "/I$output" "/Fe:$exe" "/Fo:$obj" (Join-Path $PSScriptRoot 'BgraReference.cpp')
if($LASTEXITCODE){throw 'Public BGRA reference compile failed'}
$mathHeaders=@(foreach($name in @('DirectXMath.h','DirectXMathConvert.inl','DirectXMathMatrix.inl','DirectXMathMisc.inl','DirectXMathVector.inl','DirectXPackedVector.h','DirectXPackedVector.inl')){
    $found=@($env:INCLUDE -split ';'|ForEach-Object{Join-Path $_ $name}|Where-Object{Test-Path $_})
    if($found.Count -ne 1){throw 'Ambiguous installed math header'}
    @{file=$name;sha256=(Get-FileHash $found[0]).Hash.ToLowerInvariant()}
})
Write-NewResearchFile (Join-Path $output 'build-provenance.json') ([Text.Encoding]::UTF8.GetBytes((@{
    pin='1acf4ebe6330e06b7eb52e8db63d4f824ad77427';binary_sha256=(Get-FileHash $exe).Hash.ToLowerInvariant();
    compiler_sha256=(Get-FileHash $CompilerPath).Hash.ToLowerInvariant();compiler_version=(Get-Item $CompilerPath).VersionInfo.FileVersion;
    owned_adapter_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'BgraReference.cpp')).Hash.ToLowerInvariant();
    public_branch_header_sha256=(Get-FileHash $methods).Hash.ToLowerInvariant();math_headers=$mathHeaders
}|ConvertTo-Json -Depth 8)))
'Built bounded BGRA reference with unchanged public scanline branches.'
