#!/usr/bin/env bash
# Multi-node mesh test in Docker (see docker-compose.yml). Exits non-zero on failure.
#   bash deploy/mesh-test/run.sh
set -euo pipefail
cd "$(dirname "$0")"
export MSYS_NO_PATHCONV=1   # Git Bash on Windows: keep /ip4/... arguments intact

# Placeholders until the genes exist (compose interpolates every service).
export RELAY_ID=x A_ID=x C_ID=x RELAY_PEER=x
C="docker compose -p netgene-mesh-test -f docker-compose.yml"
cleanup() { $C --profile tools down -v >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup

echo "== build"
$C build relay >/dev/null

tools() { $C --profile tools run --rm -T tools "$@" 2>/dev/null; }
id_of() { grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1; }

echo "== genes (one Master, three node sub-genes)"
MASTER=$(tools gene init --name mesh-master --json | id_of)
RELAY_ID=$(tools gene spawn --parent "$MASTER" --name relay | id_of)
A_ID=$(tools gene spawn --parent "$MASTER" --name node-a | id_of)
C_ID=$(tools gene spawn --parent "$MASTER" --name node-c | id_of)
RELAY_PEER=$(tools p2p peer-id --gene "$RELAY_ID" | tail -1 | tr -d '\r')
A_PEER=$(tools p2p peer-id --gene "$A_ID" | tail -1 | tr -d '\r')
C_PEER=$(tools p2p peer-id --gene "$C_ID" | tail -1 | tr -d '\r')
export RELAY_ID A_ID C_ID RELAY_PEER
echo "   relay=$RELAY_PEER"
echo "   a=$A_PEER"
echo "   c=$C_PEER"

echo "== start relay (lan_a + lan_b) and node-a (lan_a only)"
$C up -d relay node-a >/dev/null
for _ in $(seq 1 30); do
  $C logs node-a 2>&1 | grep -q "Reachable through relay" && break
  sleep 1
done
$C logs node-a 2>&1 | grep -q "Reachable through relay" || { echo "FAIL: node-a got no relay reservation"; $C logs; exit 1; }
echo "   node-a holds a relay reservation"

fail=0
echo "== 1. node-c (lan_b) cannot reach node-a directly"
if $C --profile tools run --rm -T --entrypoint sh node-c -c 'timeout 3 bash -c "</dev/tcp/172.29.1.20/7777"' >/dev/null 2>&1; then
  echo "FAIL: networks are not isolated"; fail=1
else
  echo "   ok (no route)"
fi

echo "== 2. encrypted gossip c -> relay -> a"
$C --profile tools run --rm -T node-c p2p broadcast --gene "$C_ID" --port 7790 --no-mdns \
  --peer /ip4/172.29.2.10/tcp/7777 "hello from C" >/dev/null 2>&1 || true
sleep 2
if $C logs node-a 2>&1 | grep "gossip from $C_PEER" | grep -q "hello from C"; then
  echo "   ok: node-a decrypted C's broadcast"
else
  echo "FAIL: node-a did not receive C's gossip"; fail=1
fi

echo "== 3. private message c -> a through a's relay circuit"
$C --profile tools run --rm -T --entrypoint timeout node-c 25 netgene p2p connect --gene "$C_ID" --port 7791 --no-mdns \
  "/ip4/172.29.2.10/tcp/7777/p2p/$RELAY_PEER/p2p-circuit/p2p/$A_PEER" --private "secret via circuit" >/dev/null 2>&1 || true
if $C logs node-a 2>&1 | grep "private from $C_PEER" | grep -q "secret via circuit"; then
  echo "   ok: node-a received the private message over the relay circuit"
else
  echo "FAIL: private message through the circuit did not arrive"; fail=1
fi

echo "== 4. an outsider gene (other Master) is refused"
OUT_MASTER=$(tools gene init --name outsider --json | id_of)
$C --profile tools run --rm -T node-c p2p broadcast --gene "$OUT_MASTER" --port 7792 --no-mdns \
  --peer /ip4/172.29.2.10/tcp/7777 "intruder" >/dev/null 2>&1 || true
sleep 1
if $C logs node-a 2>&1 | grep -q "intruder"; then
  echo "FAIL: outsider message reached node-a"; fail=1
else
  echo "   ok: outsider's message was not delivered"
fi

if [ "$fail" -ne 0 ]; then
  echo "== logs"; $C logs --tail 40
  exit 1
fi
echo "== mesh test passed"
