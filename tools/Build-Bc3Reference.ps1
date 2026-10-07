# Compile owned adapter with unchanged methods from a pinned public source.
# Downloads/installs nothing; all admitted public files and binaries are private.
param(
    [Parameter(Mandatory)][string]$PublicSourceDirectory,
    [Parameter(Mandatory)][string]$BuildDirectory,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$CompilerPath
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
$methods=Join-Path $output 'Bc3ReferenceMethods.h'
$exe=Join-Path $output 'Bc3Reference.exe'
$obj=Join-Path $output 'Bc3Reference.obj'
foreach($path in @($methods,$exe,$obj)) {if(Test-Path -LiteralPath $path){throw 'Build output already exists'}}
$text=[IO.File]::ReadAllText((Join-Path $source 'BC.cpp'))
$start=$text.IndexOf('    inline void DecodeBC1(')
$end=$text.IndexOf('    //-------------------------------------------------------------------------------------', $start)
$start3=$text.IndexOf('_Use_decl_annotations_ void DirectX::D3DXDecodeBC3(')
$end3=$text.IndexOf('_Use_decl_annotations_ void DirectX::D3DXEncodeBC3(', $start3)
if($start -lt 0 -or $end -le $start -or $start3 -lt 0 -or $end3 -le $start3) {throw 'Pinned method bounds missing'}
$header="// Microsoft DirectXTex, MIT license; exact public methods, pinned above.`nusing namespace DirectX;`nusing namespace DirectX::PackedVector;`nnamespace {`n"+
    $text.Substring($start,$end-$start)+"`n}`n"+$text.Substring($start3,$end3-$start3)
Write-NewResearchFile -Path $methods -Bytes ([Text.Encoding]::UTF8.GetBytes($header))
& $CompilerPath /nologo /std:c++17 /EHsc /W4 /WX /O2 "/I$source" "/I$output" "/Fe:$exe" "/Fo:$obj" (Join-Path $PSScriptRoot 'Bc3Reference.cpp')
if($LASTEXITCODE){throw 'Pinned BC3 reference compile failed'}
"Built explicit BC3 reference from the pinned public methods."
