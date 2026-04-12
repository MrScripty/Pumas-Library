$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$launcherCore = Join-Path $scriptDir 'scripts/launcher/cli.mjs'
$nodeCommand = Get-Command node -ErrorAction SilentlyContinue

if (-not $nodeCommand) {
    Write-Error '[launcher] error: node missing; install Node.js from https://nodejs.org/'
    exit 1
}

$env:PUMAS_LAUNCHER_DISPLAY_NAME = './launcher.ps1'
& $nodeCommand.Source $launcherCore @args
exit $LASTEXITCODE
