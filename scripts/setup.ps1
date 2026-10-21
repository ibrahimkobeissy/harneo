# setup (Windows): check, and after confirmation install, what building Harneo from source
# needs. Mirrors scripts/setup.sh. UNTESTED BY THE MAINTAINER: written from the Tauri 2
# prerequisites docs and the winget CLI; please report problems.
#   setup.ps1           install what is missing, after a confirmation
#   setup.ps1 -Dev      also the contributor gate tools
#   setup.ps1 -Check    install nothing; report what is missing, exit 1 if anything is
#   setup.ps1 -Yes      skip the confirmation
param([switch]$Dev, [switch]$Check, [switch]$Yes)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
  Write-Error 'setup: winget is required (it ships with current Windows 10/11, see https://aka.ms/getwinget).'
  exit 2
}
Write-Host 'setup: windows, package manager: winget'

$minNode = 22
if (Test-Path package.json) {
  $pj = Get-Content package.json -Raw
  if ($pj -match '"node"\s*:\s*"[^0-9"]*(\d+)') { $minNode = [int]$Matches[1] }
}

function Test-Winget($id) {
  winget list --id $id --exact --accept-source-agreements *> $null
  return ($LASTEXITCODE -eq 0)
}

$missing = @()
$plan = @()   # @{ Desc; Cmd } scriptblocks run after confirmation

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  $missing += 'rustup (Rust toolchain manager)'
  $plan += @{ Desc = 'winget install --id Rustlang.Rustup -e'; Cmd = { winget install --id Rustlang.Rustup -e --accept-package-agreements } }
}

$nodeOk = $false
if (Get-Command node -ErrorAction SilentlyContinue) {
  $v = (node --version)
  if ($v -match '^v(\d+)\.' -and [int]$Matches[1] -ge $minNode) { $nodeOk = $true }
  else { $missing += "node >= $minNode (found $v)" }
} else { $missing += "node >= $minNode" }
if (-not $nodeOk) {
  $plan += @{ Desc = 'winget install --id OpenJS.NodeJS.LTS -e'; Cmd = { winget install --id OpenJS.NodeJS.LTS -e --accept-package-agreements } }
}

# MSVC build tools with the C++ workload.
if (-not (Test-Winget 'Microsoft.VisualStudio.2022.BuildTools')) {
  $missing += 'MSVC build tools (Visual Studio 2022 Build Tools, "Desktop development with C++")'
  $plan += @{
    Desc = 'winget install Microsoft.VisualStudio.2022.BuildTools (C++ workload; may ask for administrator approval)'
    Cmd  = { winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' }
  }
}
# WebView2 runtime (preinstalled on Windows 11).
if (-not (Test-Winget 'Microsoft.EdgeWebView2Runtime')) {
  $missing += 'WebView2 runtime'
  $plan += @{ Desc = 'winget install --id Microsoft.EdgeWebView2Runtime -e'; Cmd = { winget install --id Microsoft.EdgeWebView2Runtime -e --accept-package-agreements } }
}

if ($Dev) {
  $tools = @(
    @{ Cmd = 'gitleaks'; Id = 'Gitleaks.Gitleaks' },
    @{ Cmd = 'osv-scanner'; Id = 'Google.OSVScanner' },
    @{ Cmd = 'jq'; Id = 'jqlang.jq' }
  )
  foreach ($t in $tools) {
    if (-not (Get-Command $t.Cmd -ErrorAction SilentlyContinue)) {
      $missing += "$($t.Cmd) (contributor gate tool)"
      $id = $t.Id
      $plan += @{ Desc = "winget install --id $id -e"; Cmd = { winget install --id $id -e --accept-package-agreements }.GetNewClosure() }
    }
  }
  foreach ($py in 'semgrep', 'diff-cover') {
    if (-not (Get-Command $py -ErrorAction SilentlyContinue)) {
      $missing += "$py (contributor gate tool)"
      $name = $py
      $plan += @{ Desc = "pipx install $name"; Cmd = { pipx install $name }.GetNewClosure() }
    }
  }
  if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
    $missing += 'cargo-llvm-cov (contributor gate tool)'
    $plan += @{ Desc = 'cargo install --locked cargo-llvm-cov'; Cmd = { cargo install --locked cargo-llvm-cov } }
  }
}

if ($missing.Count -eq 0) { Write-Host 'setup: everything is present.'; exit 0 }
Write-Host 'setup: missing:'
$missing | ForEach-Object { Write-Host "  - $_" }
if ($Check) { Write-Host 'setup: -Check installs nothing.'; exit 1 }

Write-Host 'setup: will run (some installers ask for administrator approval):'
$plan | ForEach-Object { Write-Host "  $($_.Desc)" }
if (-not $Yes) {
  $answer = Read-Host 'setup: proceed? [y/N]'
  if ($answer -notmatch '^[yY]') { Write-Host 'setup: cancelled, nothing changed.'; exit 1 }
}
$failed = $false
foreach ($step in $plan) {
  & $step.Cmd
  if ($LASTEXITCODE -ne 0) { Write-Host "setup: FAILED: $($step.Desc)"; $failed = $true }
}
if ($failed) { Write-Host 'setup: finished with problems; rerun with -Check.'; exit 1 }
Write-Host 'setup: done. Rerun with -Check to confirm.'
