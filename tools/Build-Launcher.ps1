# Build the launcher and give the Windows executable its product filename.
# Cargo keeps the development target name `kh2fm` for command-line runs.
param([switch]$Debug)

$ErrorActionPreference = 'Stop'
$env:PATH = (Join-Path $env:USERPROFILE '.cargo/bin') + [IO.Path]::PathSeparator + $env:PATH
$launcherProfile = if ($Debug) { 'dev' } else { 'release' }
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $launcherBuildOutput = & cargo build -p windows_shell --bin kh2fm --profile $launcherProfile --locked --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed.' }
    $launcherArtifact = $launcherBuildOutput | ForEach-Object { $_ | ConvertFrom-Json } |
        Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'kh2fm' -and $_.executable } |
        Select-Object -Last 1
    if (-not $launcherArtifact) { throw 'Cargo did not report a launcher executable.' }
    $launcherOutputPath = Join-Path (Split-Path $launcherArtifact.executable -Parent) 'KH2RW Launcher.exe'
    Copy-Item -LiteralPath $launcherArtifact.executable -Destination $launcherOutputPath -Force
    Write-Output $launcherOutputPath
} finally {
    Pop-Location
}
