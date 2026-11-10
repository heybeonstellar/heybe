# Roadmap

## Pre-phase: working testnet demo (2 weeks, from Nov 10, 2026)

### Week 1: foundation

- [ ] Design documents (this folder)
- [ ] Spikes for the open questions in [auth flows](auth-flows.md)
- [ ] Registry and account contracts
- [ ] OpenZeppelin-based test NFT
- [ ] Unit and end-to-end tests with real auth entries. Cases:
  - control moves with the NFT
  - the old owner is rejected
  - XLM balance
  - lock
  - self-ownership and self-transfer guards
  - Path B rejects anyone but the owner
- [ ] CI green

### Week 2: demo and feedback

- [ ] Testnet deployment and scripted flows. Every hash goes in `EVIDENCE.md`.
- [ ] stellar-8004 agent wallet flow on testnet
- [ ] Web demo (Path A with Freighter; Path B if the client spike succeeds)
- [ ] Demo video
- [ ] Design discussion on stellar-protocol GitHub Discussions
- [ ] Feedback round with the Stellar Türkiye chapter

## Next sprint (proposed)

- **TypeScript SDK.**
  - Computes account addresses off chain.
  - Creates accounts and runs `execute`.
  - Builds CAP-71 delegated auth entries, including the two-pass simulation.
- **Account viewer.**
  - Shows what is inside the account of any SEP-50 NFT.
  - Includes an integration proposal for the stellar-8004 explorer.
- **Contract features.**
  - Passkey (`C...`) owners in the app.
  - Marketplace-side `state` checks.
  - Path B in the UI.
  - `u128` token id support.

## Later

- Security review
- Mainnet deployment
- SEP draft with a champion, based on the stellar-protocol discussion
- Optional NFT-side extension that blocks transfers into the bound account
