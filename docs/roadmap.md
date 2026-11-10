# Roadmap

## Pre-phase: working testnet demo (2 weeks, from Nov 10, 2026)

### Week 1: foundation

- [x] Design documents (this folder)
- [x] Spikes for the open questions. Results are in [auth flows](auth-flows.md#spike-results-nov-10-2026).
- [x] Registry and account contracts
- [x] OpenZeppelin-based test NFT
- [x] Unit and end-to-end tests with real auth entries (26 tests). Covered:
  - control moves with the NFT
  - the previous owner and strangers are rejected
  - XLM balance
  - lock
  - self-ownership and self-transfer guards
  - burned NFT
  - Path B rejects anyone but the owner
- [x] CI: wasm build with stellar CLI, tests, clippy

### Week 2: demo and feedback

- [x] Testnet deployment and scripted flows. Hashes are in [EVIDENCE.md](../EVIDENCE.md).
- [x] stellar-8004 agent wallet flow on testnet
- [x] Path B with a real G-key CAP-71 delegate signature on testnet
- [ ] Web demo (Path A with Freighter; Path B if Freighter signs delegate entries)
- [ ] Demo video
- [ ] Design discussion on stellar-protocol GitHub Discussions
- [ ] Feedback round with the Stellar Türkiye chapter

## Next sprint (proposed)

- **TypeScript SDK.**
  - Computes account addresses off chain.
  - Creates accounts and runs `execute`.
  - Builds CAP-71 delegated auth entries. `scripts/lib/heybe.ts` is the starting point.
- **Account viewer.**
  - Shows what is inside the account of any SEP-50 NFT.
  - Includes an integration proposal for the stellar-8004 explorer.
- **Contract features.**
  - Passkey (`C...`) owners in the app.
  - Marketplace-side `state` checks.
  - Path B in the UI.
  - `u128` token id support.
  - Pre-authorizing deeper calls during `execute`.

## Later

- Security review
- Mainnet deployment
- SEP draft with a champion, based on the stellar-protocol discussion
- Optional NFT-side extension that blocks transfers into the bound account
