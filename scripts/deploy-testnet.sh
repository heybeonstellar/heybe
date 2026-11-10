#!/usr/bin/env bash
# Builds the contracts and deploys Heybe to Stellar testnet.
#
# Writes contract ids and wasm hashes to deployments/testnet.json.
# Requires: stellar CLI 28+, a funded identity (default: heybe-deployer).
#
#   scripts/deploy-testnet.sh            # uses identity heybe-deployer
#   DEPLOYER=my-key scripts/deploy-testnet.sh

set -euo pipefail

NETWORK="testnet"
DEPLOYER="${DEPLOYER:-heybe-deployer}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/contracts/target/wasm32v1-none/release"
OUT="$ROOT/deployments/$NETWORK.json"

if ! stellar keys address "$DEPLOYER" >/dev/null 2>&1; then
  echo "Creating and funding identity $DEPLOYER on $NETWORK"
  stellar keys generate "$DEPLOYER" --network "$NETWORK" --fund
fi

echo "Building contracts"
(cd "$ROOT/contracts" && stellar contract build >/dev/null)

echo "Uploading account wasm"
ACCOUNT_WASM_HASH=$(stellar contract upload \
  --wasm "$WASM_DIR/heybe_account.wasm" \
  --source "$DEPLOYER" --network "$NETWORK" 2>/dev/null)

echo "Deploying registry"
REGISTRY_ID=$(stellar contract deploy \
  --wasm "$WASM_DIR/heybe_registry.wasm" \
  --source "$DEPLOYER" --network "$NETWORK" 2>/dev/null \
  -- --account_wasm_hash "$ACCOUNT_WASM_HASH")

echo "Deploying test NFT"
TEST_NFT_ID=$(stellar contract deploy \
  --wasm "$WASM_DIR/heybe_test_nft.wasm" \
  --source "$DEPLOYER" --network "$NETWORK" 2>/dev/null \
  -- --uri "https://github.com/heybeonstellar/heybe/" --name "Heybe Test NFT" --symbol "HEYBE")

XLM_SAC_ID=$(stellar contract id asset --asset native --network "$NETWORK")

mkdir -p "$(dirname "$OUT")"
cat > "$OUT" <<JSON
{
  "network": "$NETWORK",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "deployer": "$(stellar keys address "$DEPLOYER")",
  "accountWasmHash": "$ACCOUNT_WASM_HASH",
  "registry": "$REGISTRY_ID",
  "testNft": "$TEST_NFT_ID",
  "xlmSac": "$XLM_SAC_ID",
  "stellar8004IdentityRegistry": "CDE3K4COIAGWNNJQQLL26SYI3KBJF5FUDHXG5FA6GYDJCG7T5V7FIWZH"
}
JSON

echo "Done. Wrote $OUT"
cat "$OUT"
