# Threat model

Token-bound accounts on EVM have a known set of failure modes. This document lists each one and states how Heybe handles it in the first version. "Residual" is what remains after the mitigation.

## 1. Sale drain (front-running a sale)

**Risk.** A seller lists an NFT whose account holds 500 USDC. The buyer pays for "NFT + 500 USDC". Just before the sale settles, the seller empties the account. The buyer receives an empty account.

**Mitigation.**
- `state()` increments on every `execute`, every successful `__check_auth` and every `lock`. A marketplace records `state` when the buyer commits and refuses to settle if it changed.
- `lock(until_ledger)` lets the seller freeze the account for the listing period. A lock can be extended but not shortened, so a buyer can trust it until it expires.

**Residual.** Both protections need marketplace support. A buyer purchasing peer to peer must check `state` and `locked_until` themselves. Assets can still be added to a locked account, but none can leave it.

## 2. Ownership cycles

**Risk.** If an account comes to own its own NFT, the owner becomes the account itself and nobody can authorize it again. The same happens if two accounts own each other's NFTs.

**Mitigation.**
- `execute` rejects transferring the bound NFT to the account itself (`SelfTransferBlocked`).
- `execute`, `lock` and `__check_auth` fail with `SelfOwned` when `owner_of` returns the account. This prevents infinite recursion and makes the state visible.

**Residual.** The owner can still transfer the NFT to the account directly through the NFT contract, and Heybe cannot see that call. Longer cycles (A owns B's NFT, B owns A's NFT) are not detected. The demo warns before such transfers. A follow-up may add an optional NFT-side extension that refuses transfers into the bound account.

## 3. Burned NFT

**Risk.** If the NFT is burned, `owner_of` fails and the account can never authorize again. Its assets are stuck. Metaplex Core has the same property.

**Mitigation.** Documented. The demo asks users to empty the account before burning.

**Residual.** Assets sent to the account of a burned NFT are unrecoverable.

## 4. Trusting the NFT contract

**Risk.** The account believes whatever `owner_of` returns. A malicious or upgradeable NFT contract can report any owner.

**Mitigation.** This assumption is inherent to token-bound accounts (ERC-6551 makes the same one). `token()` exposes the bound NFT contract, and the demo shows it on every account. Users should fund accounts only for NFT contracts they trust.

**Residual.** An account is exactly as trustworthy as its NFT contract.

## 5. Code upgrades

**Risk.** If the owner could upgrade the account code, they could swap it right before a sale.

**Mitigation.** No upgrade function in the first version. Every account runs the wasm hash stored in the registry.

**Residual.** Bugs cannot be patched in deployed accounts. Owners would have to move assets to an account created by a new registry.

## 6. Delegation misuse (Path B)

**Risk.** A caller attaches extra or wrong delegates to an auth entry for the account.

**Mitigation.** `__check_auth` accepts exactly one delegate, equal to the current owner. Anything else fails with `InvalidDelegates`. The host then verifies the owner's signature on the same payload. Nonces and expiration ledgers are enforced by the host as usual.

**Residual.** None known. This is a priority for review.

## 7. Restricted assets

**Risk.** Assets whose issuer sets `AUTH_REQUIRED` cannot be received until the issuer authorizes the account. Clawback-enabled assets can be clawed back from the account.

**Mitigation.** Documented. The demo uses XLM, which has no issuer flags.

## 8. State archival

**Risk.** If the account's instance entry expires, it is archived and the account stops working until it is restored.

**Mitigation.** Every use extends the TTL, and anyone can call `extend_ttl`. Archived entries are restorable with a fee, and balances are not lost.

## 9. Griefing through account creation

**Risk.** Anyone can call `create_account` for any NFT.

**Mitigation.** Creation grants no control. Control always comes from `owner_of`, so an account created by a stranger behaves exactly like one created by the owner. The only cost is the creator's own fee.
