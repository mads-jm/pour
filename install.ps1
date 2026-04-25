# Pour installer for Windows.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/mads-jm/pour/main/install.ps1 | iex
#
# Pin a specific version with an env var:
#   $env:POUR_VERSION = '0.2.2'
#   irm https://raw.githubusercontent.com/mads-jm/pour/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$repo = 'mads-jm/pour'
$target = 'x86_64-pc-windows-msvc'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\pour'

$Version = $env:POUR_VERSION

if ($Version) {
    $tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
} else {
    Write-Host 'Looking up latest release...'
    $tag = (Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest").tag_name
}
$num = $tag.TrimStart('v')
$assetName = "pour-$num-$target.zip"
$url = "https://github.com/$repo/releases/download/$tag/$assetName"

Write-Host "Installing pour $tag to $installDir"

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "pour-install-$([System.Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
$zipPath = Join-Path $tmp $assetName

try {
    Write-Host "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

    Write-Host 'Extracting...'
    Expand-Archive -Path $zipPath -DestinationPath $tmp -Force

    $extracted = Get-ChildItem $tmp -Directory | Where-Object { $_.Name -like 'pour-*' } | Select-Object -First 1
    if (-not $extracted) {
        throw "Archive layout unexpected: no pour-* folder found in $tmp"
    }

    if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force }
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item (Join-Path $extracted.FullName '*') $installDir -Recurse -Force
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# Add to user PATH if missing. Use [Environment]::SetEnvironmentVariable
# rather than setx — setx truncates at 1024 chars and will corrupt long PATHs.
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
$onPath = ($userPath -split ';') -contains $installDir
if (-not $onPath) {
    $newPath = if ([string]::IsNullOrEmpty($userPath)) { $installDir } else { "$userPath;$installDir" }
    [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')
    Write-Host "Added $installDir to user PATH."
    Write-Host 'Restart your terminal for the PATH change to take effect.'
} else {
    Write-Host "$installDir already on user PATH."
}

Write-Host ''
Write-Host "pour $tag installed. Run ``pour`` to get started."
