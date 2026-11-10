# Heybe

**Token-bound accounts for Soroban.** Every NFT on Stellar gets its own smart-contract account, and whoever holds the NFT controls that account. When the NFT changes hands, everything inside the account (XLM, USDC, other tokens, other NFTs) goes with it.

> Status: **design phase.** No contracts are deployed yet. This repository starts with the design documents. Contracts, tests, and a testnet demo follow over the next two weeks (see [Roadmap](docs/roadmap.md)).

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

- [Overview](docs/overview.md): problem, solution, use cases
- [Architecture](docs/architecture.md): components and how they connect
- [Interface](docs/interface.md): contract functions, errors, events (draft)
- [Auth flows](docs/auth-flows.md): Path A and Path B step by step
- [Threat model](docs/threat-model.md): known risks and mitigations
- [Demo](docs/demo.md): what the testnet demo will show
- [Roadmap](docs/roadmap.md): pre-phase, next sprint, later

## Tech stack (planned)

- Contracts: Rust, `soroban-sdk` 28.x, OpenZeppelin `stellar-tokens` 0.8.x (test NFT)
- Tooling: `stellar` CLI 28, `cargo test` with Soroban test utilities, GitHub Actions
- Demo app: TypeScript, React + Vite, `@stellar/stellar-sdk` 17.x, Freighter via Stellar Wallets Kit
- Network: Stellar testnet

## License

[MIT](LICENSE)
