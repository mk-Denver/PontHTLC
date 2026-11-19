param(
    [string]$BitcoindUrl = "http://127.0.0.1:18923",
    [string]$DaemonUrl = "http://127.0.0.1:9800",
    [string]$RpcUser = "ponthtlc",
    [string]$RpcPassword = "ponthtlc",
    [string]$Wallet = "spike",
    [long]$CapacitySatoshis = 1000000,
    [int]$Confirmations = 6,
    [int]$WalletBlocks = 101,
    [string]$NostrPubkey = ("0" * 64)
)

$ErrorActionPreference = "Stop"
$auth = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("${RpcUser}:${RpcPassword}"))

function Invoke-Rpc {
    param(
        [string]$Method,
        [object[]]$Params = @(),
        [string]$Path = "/"
    )
    $body = @{ jsonrpc = "1.0"; id = "bootstrap"; method = $Method; params = $Params } | ConvertTo-Json -Compress
    $response = Invoke-RestMethod -Method Post -Uri "$BitcoindUrl$Path" `
        -Headers @{ Authorization = "Basic $auth" } `
        -ContentType "application/json" -Body $body
    $response.result
}

function Wait-Bitcoind {
    for ($i = 0; $i -lt 60; $i++) {
        try {
            Invoke-Rpc -Method "getblockchaininfo" | Out-Null
            return $true
        } catch {
            Start-Sleep -Seconds 2
        }
    }
    return $false
}

function Wait-Daemon {
    for ($i = 0; $i -lt 90; $i++) {
        try {
            $r = Invoke-WebRequest -UseBasicParsing -Uri "$DaemonUrl/health/live" -TimeoutSec 3
            if ($r.StatusCode -eq 200) { return $true }
        } catch { }
        Start-Sleep -Seconds 2
    }
    return $false
}

function Get-ChannelStatus {
    Invoke-RestMethod -Method Get -Uri "$DaemonUrl/api/channel/status" `
        -Headers @{ "x-nostr-pubkey" = $NostrPubkey }
}

function Test-Usable {
    param($Status)
    return [bool]($Status.channels | Where-Object { $_.usable })
}

function Test-HasChannel {
    param($Status)
    return [bool]($Status.channels | Where-Object { $_.channel_id })
}

function Mine-ToWallet {
    param([int]$Blocks)
    $addr = Invoke-Rpc -Method "getnewaddress" -Path "/wallet/$Wallet"
    Invoke-Rpc -Method "generatetoaddress" -Params @($Blocks, $addr) | Out-Null
    Write-Output "mined $Blocks blocks to $Wallet wallet"
}

function Ensure-Wallet {
    try {
        Invoke-Rpc -Method "createwallet" -Params @($Wallet) | Out-Null
    } catch {
        try { Invoke-Rpc -Method "loadwallet" -Params @($Wallet) | Out-Null } catch { }
    }
}

function Fund-Wallet {
    Ensure-Wallet
    $need = $CapacitySatoshis / 100000000.0
    $balance = 0.0
    try {
        $bal = Invoke-Rpc -Method "getbalances" -Path "/wallet/$Wallet"
        $balance = [double]$bal.mine.trusted
    } catch {
        $balance = 0.0
    }
    if ($balance -lt $need) {
        Mine-ToWallet -Blocks $WalletBlocks
    }
}

if (-not (Wait-Bitcoind)) {
    Write-Error "bitcoind unavailable at $BitcoindUrl"
    exit 1
}
if (-not (Wait-Daemon)) {
    Write-Error "daemon unavailable at $DaemonUrl"
    exit 1
}

$status = Get-ChannelStatus
if (Test-Usable -Status $status) {
    Write-Output "channel already usable; nothing to do"
    exit 0
}

if (Test-HasChannel -Status $status) {
    Write-Output "channel already opening; confirming it"
    Start-Sleep -Seconds 3
    Mine-ToWallet -Blocks $Confirmations
} else {
    Fund-Wallet
    $body = @{ capacity_satoshis = $CapacitySatoshis } | ConvertTo-Json
    $open = Invoke-RestMethod -Method Post -Uri "$DaemonUrl/api/channel/open" `
        -Headers @{ "x-nostr-pubkey" = $NostrPubkey } `
        -ContentType "application/json" -Body $body
    Write-Output "channel open requested: $($open.channel_id)"
    Start-Sleep -Seconds 3
    Mine-ToWallet -Blocks $Confirmations
}

for ($i = 0; $i -lt 45; $i++) {
    $status = Get-ChannelStatus
    if (Test-Usable -Status $status) {
        Write-Output "channel usable"
        exit 0
    }
    Start-Sleep -Seconds 2
}

Write-Error "channel did not become usable"
exit 1
