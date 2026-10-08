# Finite source texture/UV/upload-input comparison. Screen shading is separate.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$InspectorLog,
    [Parameter(Mandatory)][string]$ReferenceProbe,
    [Parameter(Mandatory)][string]$GeometryReference,
    [Parameter(Mandatory)][string]$PacketReference,
    [Parameter(Mandatory)][string]$RoutingReference,
    [Parameter(Mandatory)][string]$HdProbe,
    [Parameter(Mandatory)][string]$IndexedProbe,
    [Parameter(Mandatory)][string]$IndexedOracleDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$output=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$indexedOracle=Assert-PrivateResearchDirectory $IndexedOracleDirectory $InstallRoot
foreach($file in @($InspectorLog,$ReferenceProbe,$GeometryReference,$PacketReference,$RoutingReference,$HdProbe,$IndexedProbe)){
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $file) $InstallRoot
    if((Get-Item $file).Length -gt $(if($file -eq $InspectorLog){134217728}else{33554432})){throw 'Textured verification input exceeds bound'}
}
$parseErrors=$null;$parseTokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1'),[ref]$parseTokens,[ref]$parseErrors)
if($parseErrors.Count){throw 'Comparison helper syntax differs'}
$helper=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Assert-ModelValue'},$true)
if(!$helper){throw 'Comparison helper absent'}
. ([scriptblock]::Create($helper.Extent.Text))
$reference=Get-Content $ReferenceProbe -Raw|ConvertFrom-Json
$geometry=Get-Content $GeometryReference -Raw|ConvertFrom-Json
$packets=Get-Content $PacketReference -Raw|ConvertFrom-Json
$routing=Get-Content $RoutingReference -Raw|ConvertFrom-Json
$hd=Get-Content $HdProbe -Raw|ConvertFrom-Json
$indexed=Get-Content $IndexedProbe -Raw|ConvertFrom-Json
$pixelProof=Get-Content (Join-Path $indexedOracle 'independent-model-texture-pixels.json') -Raw|ConvertFrom-Json
foreach($proof in @($geometry,$packets,$routing,$pixelProof)){
    if($proof.verdict -ne 'PASS' -or $proof.pin -ne '7a3b945c538d32c6a285128c98aefba093f52ceb' -or $proof.runtime_verified){throw 'Public finite prerequisite is unqualified'}
}
if(!$hd.hd_mip_chain_inspection_requested -or !$indexed.model_texture_pixel_inspection_requested -or
    !$hd.source_content_identity -or $hd.source_content_identity -cne $reference.source_content_identity -or
    $hd.source_content_identity -cne $indexed.source_content_identity){throw 'Pixel/geometry source identity differs'}
$events=@(Get-Content $InspectorLog|ForEach-Object{$_|ConvertFrom-Json})
$admission=@($events|Where-Object event -eq 'admitted')
$textures=@($events|Where-Object event -eq 'texture_admission')
$exits=@($events|Where-Object event -eq 'exit')
if($admission.Count -ne 1 -or !$admission[0].fresh_full_scan -or $admission[0].gameplay_enabled -or
    $admission[0].source_content_identity -cne $hd.source_content_identity -or $textures.Count -ne 1 -or
    $textures[0].runtime_verified -or $textures[0].gameplay_enabled -or $exits.Count -ne 1 -or !$exits[0].complete){throw 'Unbound/incomplete texture tour'}
Add-Type -TypeDefinition @'
using System;
public static class ModelUvExpressions {
    public static uint[] Bits(int u,int v,bool legacy) {
        float a=legacy?unchecked((short)(ushort)u)/4096.0f:(u/16)/256.0f;
        float b=legacy?unchecked((short)(ushort)v)/4096.0f:(v/16)/256.0f;
        return new[]{BitConverter.SingleToUInt32Bits(a),BitConverter.SingleToUInt32Bits(b)};
    }
}
'@
function Find-Sample($Report,$Model){
    $ids=@(0..($Report.samples.Count-1)|Where-Object{$Report.samples[$_].index -ceq $Model.index -and $Report.samples[$_].ordinal -eq $Model.ordinal})
    if($ids.Count -ne 1){throw 'Ambiguous texture source identity'}
    [int]$ids[0]
}
function Public-RgbaHash([string]$Path,[long]$Width,[long]$Height){
    $canonical=Get-ResearchCanonicalPath $Path
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $canonical) $InstallRoot
    $stream=[IO.File]::OpenRead($canonical)
    try{
        if($Width -lt 1 -or $Height -lt 1 -or $Width -gt 4096 -or $Height -gt 4096 -or $stream.Length -ne 12+$Width*$Height*4){throw 'Independent RGBA length differs'}
        $header=[byte[]]::new(12);$stream.ReadExactly($header)
        if([Text.Encoding]::ASCII.GetString($header,0,4) -ne 'RGBA' -or [BitConverter]::ToUInt32($header,4) -ne $Width -or [BitConverter]::ToUInt32($header,8) -ne $Height){throw 'Independent RGBA header differs'}
        [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()
    }finally{$stream.Dispose()}
}
$models=@{};$sourceImages=0;$sourceLevels=0;$sourcePixels=0L;$uniqueUvCorners=0L
foreach($model in $textures[0].models){
    $h=Find-Sample $hd $model;$r=Find-Sample $reference $model;$i=Find-Sample $indexed $model
    if($hd.samples[$h].sha256 -cne $reference.samples[$r].sha256 -or $hd.samples[$h].sha256 -cne $indexed.samples[$i].sha256){throw 'Original input fingerprints differ'}
    $link=@($routing.samples|Where-Object sample -eq $h)
    $g=@($geometry.samples|Where-Object{$_.sample -eq $r -and $_.bar_ordinal -eq $model.bar_ordinal})
    $p=@($packets.samples|Where-Object{$_.sample -eq $r -and $_.bar_ordinal -eq $model.bar_ordinal})
    if($link.Count -ne 1 -or $g.Count -ne 1 -or $p.Count -ne 1){throw 'Independent texture/topology/packet identity differs'}
    Assert-ModelValue $model.textures.links $link[0].routing.links 'complete texture links'
    if($model.textures.texture_bar_ordinal -ne $link[0].routing.texture_bar_ordinal){throw 'Texture BAR identity differs'}
    Assert-ModelValue @($model.textures.modes) @('original_indexed','exact_hd_base') 'explicit texture modes'
    $originals=@($indexed.samples[$i].model_texture_pixels)
    if($model.textures.images.Count -ne $originals.Count+$hd.samples[$h].replacement_samples.Count){throw 'Image denominator differs'}
    foreach($image in $model.textures.images){
        if($image.identity.kind -eq 'original'){
            $o=@($originals|Where-Object{$_.environment_ordinal -eq $image.identity.environment_ordinal -and $_.bar_ordinal -eq $model.textures.texture_bar_ordinal})
            $proof=@($pixelProof.images|Where-Object{$_.sample -eq $i -and $_.environment_ordinal -eq $image.identity.environment_ordinal -and $_.bar_ordinal -eq $model.textures.texture_bar_ordinal})
            if($o.Count -ne 1 -or $proof.Count -ne 1 -or !$proof[0].packed_indices_palette_rgba_exact -or $image.pixel_decode_profile -ne 'openkh_model_indexed_getdata_v1' -or $image.levels.Count -ne 1 -or $null -ne $image.mip_layout){throw 'Original pixel profile/denominator differs'}
            $level=$image.levels[0]
            $hash=Public-RgbaHash (Join-Path $indexedOracle $o[0].rgba_file) $o[0].width $o[0].height
            if($level.width -ne $o[0].width -or $level.height -ne $o[0].height -or $level.pixels_rgba_sha256 -cne $hash -or $hash -cne $o[0].pixels_rgba_sha256){throw 'Complete independent indexed pixels differ'}
        }elseif($image.identity.kind -eq 'replacement'){
            $replacement=@($hd.samples[$h].replacement_samples|Where-Object{$_.metadata.ordinal -eq $image.identity.replacement_ordinal})
            if($replacement.Count -ne 1 -or $image.pixel_decode_profile -cne $replacement[0].decoded_image.pixel_decode_profile){throw 'Replacement pixel profile/identity differs'}
            $expected=$replacement[0].decoded_image
            if($expected.mip_chain){
                Assert-ModelValue $image.mip_layout $expected.mip_chain.layout 'complete retained mip layout'
                if($image.levels.Count -ne $expected.mip_chain.levels.Count){throw 'Mip level denominator differs'}
                for($m=0;$m -lt $image.levels.Count;$m++){
                    if($image.levels[$m].width -ne $expected.mip_chain.layout.levels[$m].width -or $image.levels[$m].height -ne $expected.mip_chain.layout.levels[$m].height -or
                        $image.levels[$m].pixels_rgba_sha256 -cne $expected.mip_chain.levels[$m].pixels_rgba_sha256){throw 'Complete mip pixel fingerprint differs'}
                }
            }else{
                if($image.levels.Count -ne 1 -or $null -ne $image.mip_layout -or $image.levels[0].width -ne $expected.width -or $image.levels[0].height -ne $expected.height -or $image.levels[0].pixels_rgba_sha256 -cne $expected.pixels_rgba_sha256){throw 'Complete HD pixel fingerprint differs'}
            }
        }else{throw 'Unknown texture image identity'}
        $sourceImages++;$sourceLevels+=$image.levels.Count
        foreach($level in $image.levels){$sourcePixels+=[long]$level.width*$level.height}
    }
    $uvPools=@([Collections.Generic.List[object]]::new(),[Collections.Generic.List[object]]::new());$partOrdinal=0
    foreach($group in $p[0].groups|Where-Object block_offset -eq $g[0].geometry.block_offset){
        for($packet=0;$packet -lt $group.packets.Count;$packet++){
            $part=$g[0].geometry.parts[$partOrdinal];$raw=$group.packets[$packet].vpu_data
            if(!$raw -or $part.group_ordinal -ne $group.group_ordinal -or $part.packet_ordinal -ne $packet -or $raw.strips.Count -ne $part.uv_bits.Count){throw 'Raw UV packet identity differs'}
            foreach($strip in $raw.strips){$uvPools[0].Add([ModelUvExpressions]::Bits($strip[0],$strip[1],$false));$uvPools[1].Add([ModelUvExpressions]::Bits($strip[0],$strip[1],$true))}
            $partOrdinal++
        }
    }
    if($partOrdinal -ne $g[0].geometry.parts.Count -or $model.uv_routes.Count -ne 2){throw 'UV part/profile denominator differs'}
    for($profile=0;$profile -lt 2;$profile++){
        $expectedParts=@();$partOrdinal=0
        foreach($part in $g[0].geometry.parts){
            $expanded=[Collections.Generic.List[object]]::new();$cross=0
            foreach($triangle in $part.triangles){foreach($corner in $triangle){
                $expanded.Add($uvPools[$profile][[int]$corner.uv_index]);$uniqueUvCorners++
                if($corner.uv_index -lt $part.uv_base -or $corner.uv_index -ge $part.uv_base+$part.uv_bits.Count){$cross++}
            }}
            $expectedParts+=@{part_ordinal=$partOrdinal;raw_texture_index=$part.texture_index;environment_ordinal=[long]$part.texture_index -band 0xffff;expanded_uv_bits=$expanded.ToArray();cross_part_uv_corners=$cross}
            $partOrdinal++
        }
        $name=@('openkh_kkdf2_integer_truncated_uv_v1','openkh_legacy_signed16_uv_on_kkdf2_topology_v1')[$profile]
        Assert-ModelValue $model.uv_routes[$profile] (@{profile=$name;parts=$expectedParts}|ConvertTo-Json -Depth 20|ConvertFrom-Json) 'complete independent UV route'
    }
    $key="$($model.index)|$($model.ordinal)|$($model.bar_ordinal)"
    if($models.ContainsKey($key)){throw 'Duplicate admitted textured model'};$models[$key]=$model
}
$states=@{};$corners=0L;$uploadBindings=0L;$unresolvedBindings=0L
foreach($view in $events|Where-Object event -eq 'view'){
    $key="$($view.index)|$($view.ordinal)|$($view.bar_ordinal)";$model=$models[$key]
    if(!$model){throw 'Unadmitted textured view'}
    $uv=@($model.uv_routes|Where-Object profile -eq $view.uv_profile)
    if($uv.Count -ne 1 -or $view.texture_mode -notin @('original_indexed','exact_hd_base')){throw 'Unknown viewed UV/texture choice'}
    $stateKey="$key|$($view.pose_profile)|$($view.uv_profile)|$($view.texture_mode)|$([int]$view.step%4)"
    if($states.ContainsKey($stateKey)){throw 'Duplicate textured state'};$states[$stateKey]=$true
    foreach($render in $view.rendered){
        $route=$uv[0].parts[$render.part];Assert-ModelValue $render.mesh_uv_bits $route.expanded_uv_bits 'actual Mesh UV words';$corners+=$route.expanded_uv_bits.Count
        $environment=$route.environment_ordinal;$imageIndex=$null
        if($view.texture_mode -eq 'original_indexed'){
            $status='original_indexed';$imageIndex=@(0..($model.textures.images.Count-1)|Where-Object{$model.textures.images[$_].identity.kind -eq 'original' -and $model.textures.images[$_].identity.environment_ordinal -eq $environment})
            if($imageIndex.Count -ne 1){throw 'Original image binding ambiguous'};$imageIndex=$imageIndex[0]
        }else{
            $target=@($model.textures.links.ordered_targets|Where-Object{$_.kind -eq 'base' -and $_.environment_ordinal -eq $environment})
            if($target.Count -ne 1){throw 'Base target denominator differs'}
            $status=if(!$target[0].matching_replacement_ordinals.Count){'unmatched_hd_base'}elseif($target[0].matching_replacement_ordinals.Count -ne 1 -or $model.textures.links.duplicate_candidate_offsets -contains $target[0].candidate_original_asset_offset){'ambiguous_hd_base'}else{'exact_hd_base'}
            if($status -eq 'exact_hd_base'){
                $imageIndex=@(0..($model.textures.images.Count-1)|Where-Object{$model.textures.images[$_].identity.kind -eq 'replacement' -and $model.textures.images[$_].identity.replacement_ordinal -eq $target[0].matching_replacement_ordinals[0]})
                if($imageIndex.Count -ne 1){throw 'Exact replacement binding ambiguous'};$imageIndex=$imageIndex[0]
            }
        }
        Assert-ModelValue $render.texture_binding (@{status=$status;image_index=$imageIndex;environment_ordinal=$environment}|ConvertTo-Json|ConvertFrom-Json) 'texture binding without fallback'
        if($null -eq $imageIndex){
            Assert-ModelValue $render.rgb @(255,0,255) 'unresolved diagnostic color'
            if($render.image -or $render.image_upload_input_sha256){throw 'Unresolved texture silently received an image'};$unresolvedBindings++
        }else{
            $image=$model.textures.images[$imageIndex]
            Assert-ModelValue $render.image $image 'complete bound image metadata';Assert-ModelValue $render.rgb @(255,255,255) 'untinted texture color'
            if($render.image_upload_input_sha256 -cne $image.levels[0].pixels_rgba_sha256 -or $render.uploaded_mip_count -ne 1){throw 'Actual Image upload input/mip descriptor differs'};$uploadBindings++
        }
    }
}
$expected=$models.Count*2*2*2*4
if($states.Count -ne $expected -or $admission[0].expected_captures -ne $expected -or $exits[0].captures -ne $expected){throw 'Complete textured tour denominator differs'}
Write-NewResearchFile (Join-Path $output 'textured-model-input-comparison.json') ([Text.Encoding]::UTF8.GetBytes((@{
    input_verdict='PASS';models=$models.Count;states=$states.Count;source_images=$sourceImages;source_levels=$sourceLevels;source_pixels=$sourcePixels;
    independent_uv_corners=$uniqueUvCorners;actual_mesh_uv_corners=$corners;image_upload_bindings_exact=$uploadBindings;unresolved_bindings_marked_without_fallback=$unresolvedBindings;
    public_pin='7a3b945c538d32c6a285128c98aefba093f52ceb';full_public_legacy_reader_executed=$false;screen_material_verdict='NOT RUN';installed_renderer_verified=$false;runtime_verified=$false
}|ConvertTo-Json -Depth 6)))
"PASS complete texture/UV/upload inputs: $sourceImages images, $sourceLevels levels, $($states.Count) states; screen material NOT RUN."
