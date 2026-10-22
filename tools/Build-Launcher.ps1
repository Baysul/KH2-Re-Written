# Build the launcher and give the Windows executable its product filename.
# Build just the launcher and its two sibling viewers; probes are opt-in.
# Cargo cannot name a binary with a space, so the `KH2RW` artifact is renamed
# to `KH2RW Launcher.exe`; no separate `KH2RW.exe` is left beside it. The game
# runs inside the launcher executable (it restarts itself with a program name).
param([switch]$Debug, [switch]$LauncherOnly, [switch]$Skate3, [switch]$Timings)

$ErrorActionPreference = 'Stop'
$env:PATH = (Join-Path $env:USERPROFILE '.cargo/bin') + [IO.Path]::PathSeparator + $env:PATH
$launcherProfile = if ($Debug) { 'dev' } else { 'release' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $launcherTargets = @('--bin', 'KH2RW')
    if (-not $LauncherOnly) {
        $launcherTargets += @('--bin', 'model_inspector', '--bin', 'image_inspector')
    }
    if ($Skate3) {
        $launcherTargets += @('--features', 'skate3')
    }
    if ($Timings) {
        $launcherTargets += '--timings'
    }
    $launcherBuildOutput = & cargo build -p main @launcherTargets --profile $launcherProfile --locked --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
    $launcherArtifact = $launcherBuildOutput | ForEach-Object { $_ | ConvertFrom-Json } |
        Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'KH2RW' -and $_.executable } |
        Select-Object -Last 1
    if (-not $launcherArtifact) { throw 'Cargo did not report a launcher executable.' }
    $launcherOutputPath = Join-Path (Split-Path $launcherArtifact.executable -Parent) 'KH2RW Launcher.exe'
    Move-Item -LiteralPath $launcherArtifact.executable -Destination $launcherOutputPath -Force
    Write-Output $launcherOutputPath
} finally {
    Pop-Location
}
