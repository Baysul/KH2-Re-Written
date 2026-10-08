# Authored helpers for source-bound static native inspection.
function Get-NativeRuntimeFunctionRecords([byte[]]$Bytes, $Sections, $Directory, [long]$ImageSize) {
    # AMD64 PE32+ triples only. An unwind range is not a whole logical function.
    $rva=[long]$Directory.RelativeVirtualAddress;$size=[long]$Directory.Size
    if ($ImageSize -le 0 -or $ImageSize -gt [uint32]::MaxValue -or $rva -lt 0 -or $size -lt 0) {
        throw 'Invalid runtime-function image/directory bounds'
    }
    if ($rva -eq 0 -and $size -eq 0) {return}
    if ($rva -eq 0 -or $rva%4 -ne 0 -or $size -eq 0 -or $size%12 -ne 0 -or
        $size -gt 4096*12 -or $rva+$size -gt $ImageSize) {throw 'Invalid bounded AMD64 runtime-function directory'}
    $last=-1L
    for ($i=0;$i -lt $size/12;$i++) {
        $offset=Get-NativePeOffset $Sections ($rva+$i*12) 12 $Bytes.LongLength
        $begin=[long][BitConverter]::ToUInt32($Bytes,[int]$offset)
        $end=[long][BitConverter]::ToUInt32($Bytes,[int]$offset+4)
        $unwind=[long][BitConverter]::ToUInt32($Bytes,[int]$offset+8)
        if ($begin -le $last -or $begin -eq 0 -or $begin -ge $end -or $end -gt $ImageSize -or
            $unwind -eq 0 -or $unwind%4 -ne 0 -or $unwind+4 -gt $ImageSize) {throw 'Invalid ordered runtime-function triple'}
        $code=@($Sections | Where-Object {$begin -ge $_.VirtualAddress -and
            $end -le [long]$_.VirtualAddress+$_.SizeOfRawData})
        if ($code.Count -ne 1 -or !($code[0].SectionCharacteristics -band 0x20000000)) {
            throw 'Runtime-function range lacks a unique raw executable section'
        }
        $first=Get-NativePeOffset $Sections $begin 1 $Bytes.LongLength
        [void](Get-NativePeOffset $Sections ($end-1) 1 $Bytes.LongLength)
        [void](Get-NativePeOffset $Sections $unwind 4 $Bytes.LongLength)
        [pscustomobject]@{begin_rva=$begin;end_rva=$end;unwind_rva=$unwind;
            record_file_offset=$offset;code_file_offset=$first}
        $last=$begin
    }
}

function Assert-NativeSharedModuleEvidence($Manifest, $Runtime, [string]$SourceRoot,
    [string]$RelativePath, [long]$Size, [string]$Hash, [string]$SelectedIdentity) {
    if ($RelativePath -cnotin @('KINGDOM HEARTS HD 1.5+2.5 ReMIX.exe','steam_api64.dll') -or
        $Manifest.schema_version -ne 1 -or $Manifest.qualification -ne 'quarantined_unqualified' -or
        ![StringComparer]::OrdinalIgnoreCase.Equals($SourceRoot,(Get-ResearchCanonicalPath $Manifest.canonical_source_path)) -or
        $Manifest.content_identity -cnotmatch '^[0-9a-f]{64}$' -or $SelectedIdentity -cne $Manifest.content_identity -or
        $Runtime.os_module_metadata_enumerated -ne $true -or $Runtime.process_memory_modified -ne $false -or
        $Runtime.custom_game_state_or_code_memory_read -ne $false) {throw 'Shared module source/runtime scope differs'}
    $required=@('KINGDOM HEARTS II FINAL MIX.exe')
    foreach ($pair in @('first','second','third','fourth','fifth','sixth')) {
        foreach ($extension in @('hed','pkg')) {$required += "Image/dt/kh2_$pair.$extension"}
    }
    $required=@($required | Sort-Object -CaseSensitive)
    if ($Manifest.files.Count -ne $required.Count) {throw 'Shared module requires complete selected candidate commitment'}
    $memory=[IO.MemoryStream]::new();$writer=[IO.BinaryWriter]::new($memory)
    try {
        $writer.Write([Text.Encoding]::UTF8.GetBytes("kh2-source-identity-v1`0"))
        foreach ($name in $required) {
            $rows=@($Manifest.files | Where-Object relative_path -CEQ $name)
            if ($rows.Count -ne 1 -or $rows[0].size -le 0 -or $rows[0].sha256 -cnotmatch '^[0-9a-f]{64}$') {
                throw 'Shared module selected candidate record differs'
            }
            $bytes=[Text.Encoding]::UTF8.GetBytes($name)
            $writer.Write([uint64]$bytes.Length);$writer.Write($bytes);$writer.Write([uint64]$rows[0].size)
            $writer.Write([Text.Encoding]::ASCII.GetBytes($rows[0].sha256))
        }
        $identity=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($memory.ToArray())).ToLowerInvariant()
        if ($identity -cne $SelectedIdentity) {throw 'Shared module selected candidate aggregate differs'}
    } finally {$writer.Dispose();$memory.Dispose()}
    $observed=@($Runtime.processes.source_root_modules | Where-Object relative_path -CEQ $RelativePath)
    if (!$observed.Count -or @($observed | Where-Object {$_.size -ne $Size -or $_.sha256 -cne $Hash}).Count) {
        throw 'Shared module lacks matching observed runtime file identity'
    }
    return $identity
}

function Get-NativeNamedExport([byte[]]$Bytes, $Sections, $Directory, [string]$Name) {
    if (!$Name -or $Name.Length -gt 1024 -or $Directory.RelativeVirtualAddress -le 0 -or $Directory.Size -lt 40) {
        throw 'Bounded named PE export required'
    }
    $header=Get-NativePeOffset $Sections $Directory.RelativeVirtualAddress 40 $Bytes.LongLength
    $base=[BitConverter]::ToUInt32($Bytes,[int]$header+16)
    $functions=[BitConverter]::ToUInt32($Bytes,[int]$header+20)
    $names=[BitConverter]::ToUInt32($Bytes,[int]$header+24)
    $functionTable=[BitConverter]::ToUInt32($Bytes,[int]$header+28)
    $nameTable=[BitConverter]::ToUInt32($Bytes,[int]$header+32)
    $ordinalTable=[BitConverter]::ToUInt32($Bytes,[int]$header+36)
    if ($functions -gt 65536 -or $names -gt 65536) {throw 'PE export table exceeds inspection bound'}
    $matches=[Collections.Generic.List[object]]::new()
    for ($i=0;$i -lt $names;$i++) {
        $pointer=Get-NativePeOffset $Sections ([long]$nameTable+$i*4) 4 $Bytes.LongLength
        $nameRva=[BitConverter]::ToUInt32($Bytes,[int]$pointer)
        $chars=[Collections.Generic.List[byte]]::new();$ended=$false
        for ($j=0;$j -lt 1024;$j++) {
            $offset=Get-NativePeOffset $Sections ([long]$nameRva+$j) 1 $Bytes.LongLength
            $value=$Bytes[$offset]
            if ($value -eq 0) {$ended=$true;break}
            if ($value -lt 32 -or $value -ge 127) {throw 'Invalid ASCII PE export name'}
            $chars.Add($value)
        }
        if (!$ended -or !$chars.Count) {throw 'Unterminated or empty PE export name'}
        if ([Text.Encoding]::ASCII.GetString($chars.ToArray()) -cne $Name) {continue}
        $indexOffset=Get-NativePeOffset $Sections ([long]$ordinalTable+$i*2) 2 $Bytes.LongLength
        $index=[BitConverter]::ToUInt16($Bytes,[int]$indexOffset)
        if ($index -ge $functions) {throw 'PE export ordinal index exceeds table'}
        $functionOffset=Get-NativePeOffset $Sections ([long]$functionTable+$index*4) 4 $Bytes.LongLength
        $rva=[BitConverter]::ToUInt32($Bytes,[int]$functionOffset)
        if ($rva -eq 0 -or ($rva -ge $Directory.RelativeVirtualAddress -and
            [long]$rva -lt [long]$Directory.RelativeVirtualAddress+$Directory.Size)) {throw 'Missing or forwarded native export'}
        $matches.Add([pscustomobject]@{rva=[long]$rva;ordinal=[long]$base+$index})
    }
    if ($matches.Count -ne 1) {throw 'Named PE export absent or ambiguous'}
    return $matches[0]
}

function Get-NativePeOffset($Sections, [long]$Rva, [int]$Count, [long]$FileSize) {
    if ($Rva -lt 0 -or $Count -lt 1 -or $Count -gt 64) {throw 'Invalid bounded native PE span'}
    $rows=@($Sections | Where-Object {$Rva -ge $_.VirtualAddress -and
        $Rva + $Count -le [long]$_.VirtualAddress + $_.SizeOfRawData})
    if ($rows.Count -ne 1) {throw 'Native PE span is unmapped or ambiguous'}
    $offset=[long]$rows[0].PointerToRawData + $Rva - $rows[0].VirtualAddress
    if ($offset -lt 0 -or $offset + $Count -gt $FileSize) {throw 'Native PE span exceeds input'}
    return $offset
}

function Get-NativeRelativeBranchTarget([byte[]]$Bytes, [long]$InstructionRva, [long]$Offset) {
    if ($InstructionRva -lt 0 -or $InstructionRva -gt [uint32]::MaxValue -or
        $Offset -lt 0 -or $Offset + 5 -gt $Bytes.LongLength) {throw 'Relative branch span exceeds input'}
    if ($Bytes[$Offset] -notin @(0xE8,0xE9)) {throw 'Only direct rel32 CALL/JMP leads are admitted'}
    $target=$InstructionRva + 5 + [BitConverter]::ToInt32($Bytes,[int]$Offset + 1)
    if ($target -lt 0 -or $target -gt [uint32]::MaxValue) {throw 'Relative branch target exceeds PE RVA range'}
    return $target
}

function Get-NativeStaticPointerTarget([byte[]]$Bytes, $Sections, [ulong]$ImageBase,
    [long]$InstructionRva, [long]$Offset) {
    if ($InstructionRva -lt 0 -or $InstructionRva -gt [uint32]::MaxValue -or
        $Offset -lt 0 -or $Offset + 6 -gt $Bytes.LongLength) {throw 'Indirect instruction span exceeds input'}
    if ($Bytes[$Offset] -ne 0xFF -or $Bytes[$Offset+1] -notin @(0x15,0x25)) {
        throw 'Only unprefixed RIP-relative indirect CALL/JMP leads are admitted'
    }
    $slotRva=$InstructionRva + 6 + [BitConverter]::ToInt32($Bytes,[int]$Offset + 2)
    if ($slotRva -lt 0 -or $slotRva -gt [uint32]::MaxValue) {throw 'Indirect slot exceeds PE RVA range'}
    $slotOffset=Get-NativePeOffset $Sections $slotRva 8 $Bytes.LongLength
    $value=[BitConverter]::ToUInt64($Bytes,[int]$slotOffset)
    if ($value -lt $ImageBase -or $value-$ImageBase -gt [uint32]::MaxValue) {
        throw 'Static pointer is not an in-image PE RVA candidate'
    }
    return [pscustomobject]@{slot_rva=$slotRva;slot_file_offset=$slotOffset;
        slot_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
            [byte[]]$Bytes[$slotOffset..($slotOffset+7)])).ToLowerInvariant();
        static_target_rva=[long]($value-$ImageBase);
        qualification='static_file_pointer_only_not_live_destination'}
}
