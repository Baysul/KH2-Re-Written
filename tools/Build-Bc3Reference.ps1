# Compile owned adapter with unchanged methods from a pinned public source.
# Downloads/installs nothing; all admitted public files and binaries are private.
param(
    [Parameter(Mandatory)][string]$PublicSourceDirectory,
    [Parameter(Mandatory)][string]$BuildDirectory,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$CompilerPath,
    [switch]$MipChains,
    [string]$PitchSourceDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$source=Assert-PrivateResearchDirectory -Directory $PublicSourceDirectory -SourceRoot $InstallRoot
$output=Assert-PrivateResearchDirectory -Directory $BuildDirectory -SourceRoot $InstallRoot
$hashes=@{
    'BC.cpp'='7554B64224F39E68FF99F9B070394DFDDEE811593A1B3D7C8004C5C2F94A47B0'
    'BC.h'='498E86DEFA9E46F207A54AFF2FBDA814467C2D6D608E9D678FFFF5E1CD142A49'
    'DirectXTexConvert.cpp'='BD7E8BAD4F6D7A58C9E61740F4638ADF3D37DADE9EC060AB133820F707F3427A'
}
foreach($name in $hashes.Keys) {
    if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $source $name)).Hash -ne $hashes[$name]) {
        throw "Public input does not match DirectXTex 1acf4ebe6330e06b7eb52e8db63d4f824ad77427: $name"
    }
}
$pitchHeader=$null
if($MipChains) {
    if(!$PitchSourceDirectory){throw 'Explicit mip build requires pinned pitch source directory'}
    $pitchSource=Assert-PrivateResearchDirectory $PitchSourceDirectory $InstallRoot
    $pitchPath=Join-Path $pitchSource 'DirectXTexUtil.cpp'
    if((Get-FileHash $pitchPath).Hash -ne 'F61158C1ACB0C3BD7EBE9822B3050983FC7E1AD9FCA6D51070C0D474968DA380') {
        throw 'Public pitch source differs from DirectXTex pin'
    }
    $pitchText=Get-Content $pitchPath -Raw
    $multiplyStart=$pitchText.IndexOf('    inline uint64_t MulOverflow(')
    $multiplyEnd=$pitchText.IndexOf('} // namespace',$multiplyStart)
    $compute=$pitchText.IndexOf('DirectX::ComputePitch(')
    $bc3=$pitchText.IndexOf('    case DXGI_FORMAT_BC3_UNORM:', $compute)
    $normalStart=$pitchText.IndexOf('                const uint64_t nbw =', $bc3)
    $normalEnd=$pitchText.IndexOf('            }', $normalStart)
    if($multiplyStart -lt 0 -or $multiplyEnd -le $multiplyStart -or $compute -lt 0 -or $bc3 -lt $compute -or $normalStart -lt $bc3 -or $normalEnd -le $normalStart){throw 'Public pitch bounds missing'}
    $multiply=$pitchText.Substring($multiplyStart,$multiplyEnd-$multiplyStart)
    $normal=$pitchText.Substring($normalStart,$normalEnd-$normalStart)
    # Unchanged MulOverflow and complete CP_FLAGS_NONE branch body; owned wrapper.
    $pitchHeader=$multiply+"`nstatic size_t PublicBc3Pitch(size_t width,size_t height) {`nuint64_t pitch=0,slice=0;bool overflow=false;`n"+$normal+'if(overflow) throw std::runtime_error("public pitch overflow");return static_cast<size_t>(slice);}'
}
$methods=Join-Path $output 'Bc3ReferenceMethods.h'
$stem=if($MipChains){'Bc3MipReference'}else{'Bc3Reference'}
$exe=Join-Path $output ($stem+'.exe')
$obj=Join-Path $output ($stem+'.obj')
foreach($path in @($methods,$exe,$obj)) {if(Test-Path -LiteralPath $path){throw 'Build output already exists'}}
if($MipChains -and (Test-Path (Join-Path $output 'Bc3PitchBranch.h'))){throw 'Pitch header exists'}
$text=[IO.File]::ReadAllText((Join-Path $source 'BC.cpp'))
$start=$text.IndexOf('    inline void DecodeBC1(')
$end=$text.IndexOf('    //-------------------------------------------------------------------------------------', $start)
$start3=$text.IndexOf('_Use_decl_annotations_ void DirectX::D3DXDecodeBC3(')
$end3=$text.IndexOf('_Use_decl_annotations_ void DirectX::D3DXEncodeBC3(', $start3)
if($start -lt 0 -or $end -le $start -or $start3 -lt 0 -or $end3 -le $start3) {throw 'Pinned method bounds missing'}
$header="// Microsoft DirectXTex, MIT license; exact public methods, pinned above.`nusing namespace DirectX;`nusing namespace DirectX::PackedVector;`nnamespace {`n"+
    $text.Substring($start,$end-$start)+"`n}`n"+$text.Substring($start3,$end3-$start3)
Write-NewResearchFile -Path $methods -Bytes ([Text.Encoding]::UTF8.GetBytes($header))
if($MipChains){Write-NewResearchFile (Join-Path $output 'Bc3PitchBranch.h') ([Text.Encoding]::UTF8.GetBytes($pitchHeader))}
$defines=@(if($MipChains){'/DBC3_MIP_CHAINS'})
& $CompilerPath /nologo /std:c++17 /EHsc /W4 /WX /O2 @defines "/I$source" "/I$output" "/Fe:$exe" "/Fo:$obj" (Join-Path $PSScriptRoot 'Bc3Reference.cpp')
if($LASTEXITCODE){throw 'Pinned BC3 reference compile failed'}
if($MipChains){
    $mathHeaders=@(foreach($name in @('DirectXMath.h','DirectXMathConvert.inl','DirectXMathMatrix.inl','DirectXMathMisc.inl','DirectXMathVector.inl','DirectXPackedVector.h','DirectXPackedVector.inl')){
        $found=@($env:INCLUDE -split ';'|ForEach-Object{Join-Path $_ $name}|Where-Object{Test-Path $_})
        if($found.Count -ne 1){throw 'Ambiguous installed math header'}
        @{file=$name;sha256=(Get-FileHash $found[0]).Hash.ToLowerInvariant()}
    })
    Write-NewResearchFile (Join-Path $output 'build-provenance.json') ([Text.Encoding]::UTF8.GetBytes((@{
        pin='1acf4ebe6330e06b7eb52e8db63d4f824ad77427';mode='complete_bc3_mips';
        binary_sha256=(Get-FileHash $exe).Hash.ToLowerInvariant();compiler_sha256=(Get-FileHash $CompilerPath).Hash.ToLowerInvariant();
        compiler_version=(Get-Item $CompilerPath).VersionInfo.FileVersion;math_headers=$mathHeaders;
        owned_adapter_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'Bc3Reference.cpp')).Hash.ToLowerInvariant();
        public_method_header_sha256=(Get-FileHash $methods).Hash.ToLowerInvariant();
        public_pitch_header_sha256=(Get-FileHash (Join-Path $output 'Bc3PitchBranch.h')).Hash.ToLowerInvariant();
        public_source_hashes=$hashes;pitch_source_sha256=(Get-FileHash $pitchPath).Hash.ToLowerInvariant();
        public_pitch_mode='unchanged ComputePitch BC3 CP_FLAGS_NONE branch body and MulOverflow; owned wrapper'
    }|ConvertTo-Json -Depth 8)))
}
"Built explicit BC3 reference from the pinned public methods."
