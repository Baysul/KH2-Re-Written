# Narrow instruction operands and readable-span admission; no process access here.
function Get-NativeRegisterMemoryOperand([byte[]]$Instruction) {
    if (!$Instruction -or $Instruction.Length -lt 1 -or $Instruction.Length -gt 15) {
        throw 'Complete bounded instruction required'
    }
    if ([Convert]::ToHexString($Instruction) -ceq '488B01') {
        return @{base_register='RCX';displacement=0;width=8;kind='load_RAX'}
    }
    if ($Instruction.Length -eq 4 -and $Instruction[0] -eq 0x48 -and
        $Instruction[1] -eq 0xff -and $Instruction[2] -eq 0x60) {
        $displacement=[int]$Instruction[3]
        if ($displacement -ge 128) {$displacement-=256}
        return @{base_register='RAX';displacement=$displacement;width=8;kind='indirect_jump_slot'}
    }
    return $null
}

function Get-NativeIndirectReadAddress([uint64]$Word,[int]$Displacement) {
    # Decimal arithmetic avoids wrapping a queried unsigned word into an address.
    if (!$Word -or $Word -gt [long]::MaxValue -or $Displacement -lt -128 -or $Displacement -gt 127) {
        throw 'Unqualified indirect base or displacement'
    }
    $address=[decimal]$Word+[decimal]$Displacement
    if ($address -le 0 -or $address -gt ([decimal][long]::MaxValue-8)) {
        throw 'Indirect eight-byte span exceeds signed address bounds'
    }
    return [long]$address
}

function Assert-NativeReadableSpan($Region,[long]$Address,[int]$Count,[uint32]$ExpectedType,[uint64]$AllocationBase) {
    if ($Address -le 0 -or $Count -lt 1 -or $Count -gt 64 -or $Address -gt [long]::MaxValue-$Count -or
        !$AllocationBase -or $ExpectedType -notin @(0x20000,0x1000000)) {
        throw 'Explicit bounded private/image selection required'
    }
    $basic=[uint32]$Region.Protect -band 0xff
    $end=[decimal]$Region.BaseAddress+[decimal]$Region.RegionSize
    if ($Region.State -ne 0x1000 -or $Region.Type -ne $ExpectedType -or
        $Region.AllocationBase -ne $AllocationBase -or $Region.AllocationBase -gt $Region.BaseAddress -or
        $basic -notin @(2,4,8,0x20,0x40,0x80) -or ([uint32]$Region.Protect -band 0x100) -ne 0 -or
        $Region.RegionSize -eq 0 -or $end -gt [decimal][uint64]::MaxValue -or
        [decimal]$Address -lt [decimal]$Region.BaseAddress -or [decimal]$Address+$Count -gt $end) {
        throw 'Selected indirect span lacks the expected committed readable region'
    }
}
