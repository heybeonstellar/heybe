# Overview

## The problem

Stellar NFTs cannot own things. An NFT is an entry in an NFT contract that maps a token id to an owner address. Balances of XLM, USDC and other tokens belong to accounts (`G...` addresses) or contracts (`C...` addresses), never to a token id.

That leaves a gap whenever an NFT stands for something that should carry its own assets:

- **AI agents.** [stellar-8004](https://github.com/trionlabs/stellar-8004) registers agents as NFTs on Stellar mainnet. An agent's money sits in a separate `agentWallet` address. When the agent NFT is transferred, the identity registry clears `agentWallet`, but the funds stay with whoever holds the old wallet key. The new owner gets the identity and reputation, but not the money.
- **Game characters.** A character cannot carry its inventory and gold to a new player.
- **Deeds and real-world assets.** Rental income cannot accumulate "inside" a property token and move with it on sale.

## The solution

Heybe gives every NFT a smart-contract account whose address is derived from the NFT `(nft_contract, token_id)`. The account:

- holds any Stellar asset through the Stellar Asset Contract, and other NFTs
- calls other contracts (payments, DeFi, registries)
- accepts instructions only from the NFT's **current** owner, which it reads from the NFT contract on every request

Transferring the NFT is the only step needed to transfer the account and everything in it. The NFT contract does not change, and existing NFTs work without migration.

## Use cases

1. **Agent wallets (flagship).** Bind a Heybe account as a stellar-8004 agent's wallet. Payments the agent earns, subscriptions it pays and x402 spending all live in the account. Selling or handing over the agent moves the funds too.
2. **Game inventories.** A character NFT carries its items and currency.
3. **Event tickets.** A ticket holds badges and perks. [CrowdPass](https://github.com/crowdpass-live/tokenbound_impl) already does this inside its own platform, so it is not Heybe's focus.
4. **Deeds with income.** A deed NFT accumulates rent and transfers it with the property.
5. **Memberships.** A membership NFT collects the member's rewards.

## Why Stellar, why now

- **CAP-71 is live.** Protocol 27 added `delegate_account_auth`, which lets a custom account hand authentication to another address with a single auth entry. The CAP status is Final. `soroban-sdk` exposes it as `env.custom_account().delegate_auth()`.
- **Contracts hold assets without trustlines.** The Stellar Asset Contract stores contract balances in contract data, so a Heybe account can receive XLM or USDC directly. Assets whose issuer sets `AUTH_REQUIRED` still need the issuer to authorize the account.
- **Deterministic deployment.** A contract deployed by another contract with a salt has an address that can be computed in advance, so the account address is known before it exists.
- **A common NFT interface.** OpenZeppelin's Stellar NFTs implement SEP-50 with `u32` token ids, and stellar-8004 uses them.
- **Client support.** `@stellar/stellar-sdk` 17.4 includes the CAP-71 XDR types (`SorobanDelegateSignature`, `SorobanAddressCredentialsWithDelegates`).

## Prior art

| | Chain | Notes |
|---|---|---|
| [ERC-6551](https://eips.ethereum.org/EIPS/eip-6551) / Tokenbound | EVM | The original standard. Registry plus account implementation. |
| [Metaplex Core Execute](https://developers.metaplex.com/core/execute-asset-signing) | Solana | Each Core asset has a signer PDA that can hold SOL and tokens. |
| [cw-tba](https://github.com/MegaRockLabs/cw-tba) | CosmWasm | Token-bound accounts for CW-721 NFTs. |
| [CrowdPass TBA](https://github.com/crowdpass-live/tokenbound_impl) | Stellar | Registry and account inside a ticketing app. Uses `u128` ids and `require_auth_for_args` in `__check_auth`. |

Heybe is the first attempt at a general, SEP-50-compatible primitive on Stellar that uses CAP-71 delegation. The goal is to turn it into a SEP once the design has had community review.

## Non-goals for the first version

- `u128` token ids (planned next)
- passkey (`C...`) owners in the demo app (the contract design already allows them)
- upgradeable account code
- mainnet deployment and audit
