# Migrate legacy Pour data into the centralized ~/.pour/ layout.
#
# Before:
#   %APPDATA%\pour\config.toml
#   %APPDATA%\pour\secrets.toml
#   %LOCALAPPDATA%\pour\presets.json
#   %LOCALAPPDATA%\pour\state.json
#   %LOCALAPPDATA%\pour\history.jsonl
#   %LOCALAPPDATA%\pour\history-summary.json
#
# After:
#   ~/.pour/config.toml
#   ~/.pour/secrets.toml
#   ~/.pour/presets.json
#   ~/.pour/cache/state.json
#   ~/.pour/cache/history.jsonl
#   ~/.pour/cache/history-summary.json
#
# Run:  pwsh scripts/migrate_pour_home.ps1

$ErrorActionPreference = 'Stop'

$HomeDir   = [Environment]::GetFolderPath('UserProfile')
$PourHome  = Join-Path $HomeDir '.pour'
$PourCache = Join-Path $PourHome 'cache'

$OldConfig = Join-Path $env:APPDATA      'pour'
$OldCache  = Join-Path $env:LOCALAPPDATA 'pour'

New-Item -ItemType Directory -Force -Path $PourHome  | Out-Null
New-Item -ItemType Directory -Force -Path $PourCache | Out-Null

$moves = @(
    @{ Src = Join-Path $OldConfig 'config.toml';           Dst = Join-Path $PourHome  'config.toml' }
    @{ Src = Join-Path $OldConfig 'secrets.toml';          Dst = Join-Path $PourHome  'secrets.toml' }
    @{ Src = Join-Path $OldCache  'presets.json';          Dst = Join-Path $PourHome  'presets.json' }
    @{ Src = Join-Path $OldCache  'state.json';            Dst = Join-Path $PourCache 'state.json' }
    @{ Src = Join-Path $OldCache  'history.jsonl';         Dst = Join-Path $PourCache 'history.jsonl' }
    @{ Src = Join-Path $OldCache  'history-summary.json';  Dst = Join-Path $PourCache 'history-summary.json' }
)

$moved   = 0
$skipped = 0

foreach ($m in $moves) {
    if (-not (Test-Path $m.Src)) {
        Write-Host "skip (missing): $($m.Src)"
        $skipped++
        continue
    }
    if (Test-Path $m.Dst) {
        Write-Host "skip (dest exists): $($m.Dst)"
        $skipped++
        continue
    }
    Move-Item -Path $m.Src -Destination $m.Dst
    Write-Host "moved: $($m.Src) -> $($m.Dst)"
    $moved++
}

# Remove old dirs if they're empty now.
foreach ($dir in @($OldConfig, $OldCache)) {
    if ((Test-Path $dir) -and -not (Get-ChildItem -Force $dir)) {
        Remove-Item $dir
        Write-Host "removed empty: $dir"
    }
}

Write-Host ""
Write-Host "Done. moved=$moved skipped=$skipped"
Write-Host "New root: $PourHome"
