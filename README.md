# Heybe

**Token-bound accounts for Soroban.** Every NFT on Stellar gets its own smart-contract account, and whoever holds the NFT controls that account. When the NFT changes hands, everything inside the account (XLM, USDC, other tokens, other NFTs) goes with it.

> Status: **running on Stellar testnet.** The registry and account contracts are deployed. Both authorization paths, including CAP-71 delegation with a real wallet key, are proven with public transactions in [EVIDENCE.md](EVIDENCE.md). A web demo is next (see [Roadmap](docs/roadmap.md)). Testnet only: not audited, not on mainnet.

**Example: an AI agent whose money moves with it.** A stellar-8004 agent NFT gets a Heybe account. The account is bound as the agent's wallet, and the agent is transferred. The new owner then spends and re-binds the same account, and the previous owner is rejected on chain. [See the transactions](EVIDENCE.md#stellar-8004-flow-an-agent-nft-with-its-own-wallet).

## Why

On Stellar today an NFT is a record in a contract: "token 7 belongs to Ayşe". The NFT itself cannot hold anything. Assets belong to accounts, so a game character, a property deed or an AI agent cannot be handed over together with what it owns.

The clearest case is [stellar-8004](https://github.com/trionlabs/stellar-8004), which registers AI agents as NFTs on Stellar mainnet. Each agent has an `agentWallet` field, but it only points at another address. When the agent NFT is transferred, the registry clears that field, and the funds stay with whoever controls the old wallet key.

Heybe fixes this by deriving an account from the NFT itself:

```
NFT (contract, token_id)  ──derives──▶  Heybe account (fixed address)
        │                                      │
   owner_of(id)  ◀──── asks on every auth ─────┘
```

## How it works

Heybe has two contracts. Existing NFT contracts and tokens stay untouched.

- **Registry.** Computes a deterministic account address from `(nft_contract, token_id)` and deploys the account there. Anyone can deploy an account for any NFT. Doing so grants no control.
- **Account.** Knows which NFT it is bound to. On every authorization it asks the NFT contract for `owner_of(token_id)` and accepts only that owner. It never stores the owner, so control moves the moment the NFT moves.

The account can be used in two ways:

| | Path A: `execute` | Path B: CAP-71 custom account |
|---|---|---|
| How | The owner signs a call to `account.execute(target, fn, args)` | Any contract calls `require_auth` on the account; its `__check_auth` delegates to the current owner with `delegate_auth` |
| Wallet support | Any Stellar wallet today | Needs wallets that sign delegated auth entries ([CAP-71](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0071.md), Protocol 27) |
| Used for | Payments, contract calls, binding the account as an agent wallet | Using the account anywhere an `Address` must authorize |

Details: [Architecture](docs/architecture.md) · [Auth flows](docs/auth-flows.md) · [Interface](docs/interface.md)

## What's different from prior work

| | ERC-6551 (EVM) | CrowdPass TBA (Stellar) | Heybe |
|---|---|---|---|
| Scope | Standard + reference implementation | Part of an event-ticketing app | Standalone primitive, SEP draft planned |
| NFT compatibility | ERC-721 | Its own `u128`-id ticket NFT | SEP-50 / OpenZeppelin NFTs (`u32` ids) |
| Authorization | EVM signatures | `require_auth_for_args` inside `__check_auth` (pre-CAP-71) | CAP-71 `delegate_auth` plus a plain `execute` path |

[CrowdPass](https://github.com/crowdpass-live/tokenbound_impl) ships a Soroban token-bound account inside its ticketing platform. Heybe builds on the same idea as a general primitive. Because it targets standard SEP-50 NFTs, it works with collections such as stellar-8004 agents that CrowdPass's implementation cannot read.

## Documents

- [Evidence](EVIDENCE.md): every claim with its testnet transaction
- [Overview](docs/overview.md): problem, solution, use cases
- [Architecture](docs/architecture.md): components and how they connect
- [Interface](docs/interface.md): contract functions, errors, events (draft)
- [Auth flows](docs/auth-flows.md): Path A and Path B step by step
- [Threat model](docs/threat-model.md): known risks and mitigations
- [Demo](docs/demo.md): what the testnet demo will show
- [Roadmap](docs/roadmap.md): pre-phase, next sprint, later

## Testnet deployment

| Contract | Address |
|---|---|
| Registry | [`CA6H2DQV…UX4QO`](https://stellar.expert/explorer/testnet/contract/CA6H2DQVL6RN4UBAHHWZQASWDUIYA4O3CUZIUKD63D4QIIEUHGJUX4QO) |
| Test NFT | [`CDVWUFUP…OQN3V`](https://stellar.expert/explorer/testnet/contract/CDVWUFUPRRCPICQJD2FBJMXXZRNMZWMHSKCE7QOTCFMFW2OQYV5OQN3V) |
| Account wasm hash | `2f8d6e81…aabd4b0` |

Full ids are in [`deployments/testnet.json`](deployments/testnet.json).

## Repository layout

| Path | What it is |
|---|---|
| `contracts/account` | The Heybe account contract (Path A `execute`, Path B `__check_auth`, lock, guards) |
| `contracts/registry` | Deterministic addresses and permissionless deployment |
| `contracts/test-nft` | OpenZeppelin SEP-50 NFT with open minting, for tests and the demo |
| `scripts/` | Testnet deploy script and the flows that produce [EVIDENCE.md](EVIDENCE.md) |
| `deployments/` | Testnet contract ids and recorded evidence |
| `docs/` | Design documents |

## Run it yourself

Requires Rust with the `wasm32v1-none` target, `stellar` CLI 28+, and Node 22.18+.

```bash
# build and test the contracts
cd contracts && stellar contract build && cargo test

# deploy your own copy to testnet (creates and funds a heybe-deployer identity)
scripts/deploy-testnet.sh

# run the scripted flows (needs funded identities heybe-ali, heybe-ayse, heybe-bob)
for k in heybe-ali heybe-ayse heybe-bob; do stellar keys generate $k --network testnet --fund; done
cd scripts && npm install && npm run flows && npm run flow:8004 && npm run evidence
```

## Tech stack

- Contracts: Rust, `soroban-sdk` 28.0, OpenZeppelin `stellar-tokens` 0.8 (test NFT)
- Tooling: `stellar` CLI 28, `cargo test` with Soroban test utilities, GitHub Actions
- Scripts: TypeScript on Node (built-in type stripping), `@stellar/stellar-sdk` 17.4
- Demo app (next): TypeScript, React + Vite, Freighter via Stellar Wallets Kit
- Network: Stellar testnet

## License

[MIT](LICENSE)
