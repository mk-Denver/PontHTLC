#!/bin/sh
# Opens a channel between the daemon's internal buyer and seller nodes.
# Idempotent: if a usable channel already exists it exits without changes.
# Requires only curl (no jq). Works from the host or from a container.

set -eu

BITCOIND_URL="${BITCOIND_URL:-http://127.0.0.1:18923}"
DAEMON_URL="${DAEMON_URL:-http://127.0.0.1:9800}"
RPC_USER="${RPC_USER:-ponthtlc}"
RPC_PASS="${RPC_PASS:-ponthtlc}"
WALLET="${WALLET:-spike}"
CAPACITY_SATOSHIS="${CAPACITY_SATOSHIS:-1000000}"
CONFIRMATIONS="${CONFIRMATIONS:-6}"
WALLET_BLOCKS="${WALLET_BLOCKS:-101}"
NOSTR_PUBKEY="${NOSTR_PUBKEY:-0000000000000000000000000000000000000000000000000000000000000000}"

rpc() {
    method="$1"
    params="${2:-[]}"
    curl -fsS --max-time 15 --user "$RPC_USER:$RPC_PASS" \
        -H 'content-type: application/json' \
        --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"bootstrap\",\"method\":\"$method\",\"params\":$params}" \
        "$BITCOIND_URL/"
}

rpc_wallet() {
    method="$1"
    params="${2:-[]}"
    curl -fsS --max-time 15 --user "$RPC_USER:$RPC_PASS" \
        -H 'content-type: application/json' \
        --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"bootstrap\",\"method\":\"$method\",\"params\":$params}" \
        "$BITCOIND_URL/wallet/$WALLET"
}

string_result() {
    sed -n 's/.*"result":"\([^"]*\)".*/\1/p' | head -n 1
}

number_field() {
    sed -n "s/.*\"$1\":\([0-9][0-9.eE+-]*\).*/\1/p" | head -n 1
}

wait_bitcoind() {
    i=0
    while [ "$i" -lt 60 ]; do
        if rpc getblockchaininfo '[]' >/dev/null 2>&1; then
            return 0
        fi
        i=$((i + 1))
        sleep 2
    done
    return 1
}

wait_daemon() {
    i=0
    while [ "$i" -lt 90 ]; do
        if curl -fsS --max-time 3 "$DAEMON_URL/health/live" >/dev/null 2>&1; then
            return 0
        fi
        i=$((i + 1))
        sleep 2
    done
    return 1
}

status_json() {
    curl -fsS --max-time 15 -H "x-nostr-pubkey: $NOSTR_PUBKEY" "$DAEMON_URL/api/channel/status"
}

is_usable() {
    status_json | grep -q '"usable":true'
}

has_channel() {
    status_json | grep -q '"channel_id"'
}

ensure_wallet() {
    rpc createwallet "[\"$WALLET\"]" >/dev/null 2>&1 ||
        rpc loadwallet "[\"$WALLET\"]" >/dev/null 2>&1 ||
        true
}

mine_to_wallet() {
    blocks="$1"
    addr=$(rpc_wallet getnewaddress '[]' | string_result)
    if [ -z "$addr" ]; then
        echo "could not obtain a wallet address" >&2
        return 1
    fi
    rpc generatetoaddress "[$blocks,\"$addr\"]" >/dev/null
    echo "mined $blocks blocks to $WALLET wallet"
}

fund_wallet() {
    ensure_wallet
    trusted=$(rpc_wallet getbalances '[]' 2>/dev/null | number_field trusted || true)
    [ -z "$trusted" ] && trusted=0
    need=$(awk -v s="$CAPACITY_SATOSHIS" 'BEGIN { printf "%.8f", s / 100000000 }' 2>/dev/null || echo 0.01)
    enough=$(awk -v t="$trusted" -v n="$need" 'BEGIN { print (t >= n) ? "1" : "0" }' 2>/dev/null || echo 0)
    if [ "$enough" != "1" ]; then
        mine_to_wallet "$WALLET_BLOCKS"
    fi
}

mine_confirmations() {
    sleep 3
    mine_to_wallet "$CONFIRMATIONS"
}

wait_usable() {
    i=0
    while [ "$i" -lt 45 ]; do
        if is_usable; then
            return 0
        fi
        i=$((i + 1))
        sleep 2
    done
    return 1
}

open_channel() {
    body="{\"capacity_satoshis\":$CAPACITY_SATOSHIS}"
    resp=$(curl -fsS --max-time 60 -X POST \
        -H "x-nostr-pubkey: $NOSTR_PUBKEY" \
        -H 'content-type: application/json' \
        --data-binary "$body" \
        "$DAEMON_URL/api/channel/open")
    echo "channel open requested: $resp"
}

main() {
    if ! wait_bitcoind; then
        echo "bitcoind unavailable at $BITCOIND_URL" >&2
        exit 1
    fi
    if ! wait_daemon; then
        echo "daemon unavailable at $DAEMON_URL" >&2
        exit 1
    fi

    if is_usable; then
        echo "channel already usable; nothing to do"
        exit 0
    fi

    if has_channel; then
        echo "channel already opening; confirming it"
        mine_confirmations
    else
        fund_wallet
        open_channel
        mine_confirmations
    fi

    if wait_usable; then
        echo "channel usable"
    else
        echo "channel did not become usable" >&2
        exit 1
    fi
}

main "$@"
