<#
.SYNOPSIS
    Bootstraps Windows build prerequisites for PontHTLC.
.DESCRIPTION
    Checks for and installs (if missing):
      - Rust toolchain (via rustup)
      - Visual Studio Build Tools / Community with MSVC + Windows SDK
      - Docker Desktop
    Designed for fellow contributors on a fresh Windows machine.
    Run from an elevated PowerShell session.
#>

[CmdletBinding()]
param(
    [switch] $SkipDocker
)

$ErrorActionPreference = "Stop"

function Write-Step([string] $msg) {
    Write-Host ""
    Write-Host "== $msg" -ForegroundColor Cyan
}

function Test-Command([string] $name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

# --- 1. Rust toolchain ------------------------------------------------------

Write-Step "Checking Rust toolchain"

if (Test-Command "cargo") {
    Write-Host "  Rust/Cargo found: $(cargo --version)" -ForegroundColor Green
} else {
    Write-Host "  Rust not found — installing via rustup..." -ForegroundColor Yellow
    Invoke-RestMethod https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
    & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    Write-Host "  Rust installed: $(cargo --version)" -ForegroundColor Green
}

# --- 2. Visual Studio / MSVC + Windows SDK ----------------------------------

Write-Step "Checking Visual Studio with MSVC + Windows SDK"

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsFound = $false

if (Test-Path $vswhere) {
    $vsInstall = & $vswhere -latest -property installationPath 2>$null
    if ($vsInstall) {
        $vcvars = Join-Path $vsInstall "VC\Auxiliary\Build\vcvars64.bat"
        if (Test-Path $vcvars) {
            Write-Host "  Visual Studio found at: $vsInstall" -ForegroundColor Green
            $vsFound = $true
        }
    }
}

if (-not $vsFound) {
    Write-Host "  Visual Studio not found — installing Build Tools..." -ForegroundColor Yellow
    $installerDir = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer"
    if (-not (Test-Path $installerDir)) {
        Write-Host "  Downloading VS Installer bootstrapper..." -ForegroundColor Yellow
        $bootstrapper = "$env:TEMP\vs_bootstrapper.exe"
        Invoke-WebRequest "https://aka.ms/vs/17/release/vs_BuildTools.exe" -OutFile $bootstrapper
        & $bootstrapper --quiet --wait
    }
    $setup = Join-Path $installerDir "setup.exe"
    if (Test-Path $setup) {
        Write-Host "  Installing MSVC + Windows SDK (this may take several minutes)..." -ForegroundColor Yellow
        & $setup modify --installPath $vsInstall `
            --add Microsoft.VisualStudio.Workload.VCTools `
            --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
            --add Microsoft.VisualStudio.Component.Windows11SDK.26100 `
            --quiet --norestart
        Write-Host "  Visual Studio Build Tools installed." -ForegroundColor Green
    } else {
        Write-Host "  ERROR: Could not locate VS Installer setup.exe" -ForegroundColor Red
        Write-Host "  Please install Visual Studio Build Tools manually:" -ForegroundColor Yellow
        Write-Host "  https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor Yellow
    }
}

# --- 3. Docker Desktop ------------------------------------------------------

if (-not $SkipDocker) {
    Write-Step "Checking Docker Desktop"

    if (Test-Command "docker") {
        Write-Host "  Docker found: $(docker --version)" -ForegroundColor Green
    } else {
        Write-Host "  Docker not found." -ForegroundColor Yellow
        Write-Host "  Please install Docker Desktop: https://www.docker.com/products/docker-desktop" -ForegroundColor Yellow
        Write-Host "  Or run this script with -SkipDocker to skip this check." -ForegroundColor Yellow
    }
}

# --- Done -------------------------------------------------------------------

Write-Step "Setup complete"
Write-Host "  To build the spike, run:" -ForegroundColor Cyan
Write-Host '    .\scripts\spike.ps1 -Scenario happy' -ForegroundColor White
Write-Host ""