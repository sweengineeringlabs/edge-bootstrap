# Bootstrap
$ErrorActionPreference = 'Stop'
$scmRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scmRoot
Write-Host "==> Installing git hooks"
git -C $repoRoot config core.hooksPath scm/scripts/hooks
Write-Host "==> Fetching dependencies"
Push-Location $scmRoot
cargo fetch --locked
Pop-Location
Write-Host "Bootstrap complete."
