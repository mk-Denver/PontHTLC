param(
    [string]$Scenario = "happy"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot

docker compose up -d bitcoind
if ($LASTEXITCODE -ne 0) {
    Write-Output "docker compose failed to start bitcoind"
    exit 1
}

cargo run -p pontldk-spike -- --scenario $Scenario
exit $LASTEXITCODE
