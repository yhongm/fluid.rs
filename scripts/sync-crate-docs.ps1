$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$crateDir = Join-Path $repoRoot 'crates/fluid_core'

$pairs = @(
    @{ Source = Join-Path $repoRoot 'README.md'; Destination = Join-Path $crateDir 'README.md' },
    @{ Source = Join-Path $repoRoot 'api.md'; Destination = Join-Path $crateDir 'api.md' },
    @{ Source = Join-Path $repoRoot 'demo.gif'; Destination = Join-Path $crateDir 'demo.gif' }
)

foreach ($pair in $pairs) {
    if (-not (Test-Path $pair.Source)) {
        throw "Source file not found: $($pair.Source)"
    }

    Copy-Item -Path $pair.Source -Destination $pair.Destination -Force
    Write-Host "Synced $($pair.Source) -> $($pair.Destination)"
}

Write-Host 'Done. Crate docs are now synchronized with repository docs.'
