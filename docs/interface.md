# Contract interface (draft v0)

> Draft. This is the target interface for the first implementation. Names and error codes may change before the contracts are tagged. Token ids are `u32`, matching OpenZeppelin's SEP-50 implementation.

## Registry

```rust
/// Deploys one Heybe account per (nft, token_id).
pub trait HeybeRegistry {
    /// Stores the wasm hash of the account contract. Called once at deploy.
    fn __constructor(env: Env, account_wasm_hash: BytesN<32>);

    /// Deterministic account address for an NFT. Works before the account exists.
    fn account_of(env: Env, nft: Address, token_id: u32) -> Address;

    /// Deploys the account for an NFT and returns its address.
    /// Permissionless: anyone may call it. Creating an account grants no control.
    /// Errors: AlreadyCreated.
    fn create_account(env: Env, nft: Address, token_id: u32) -> Address;

    /// True once create_account has succeeded for this NFT.
    fn is_created(env: Env, nft: Address, token_id: u32) -> bool;

    /// The wasm hash every account is deployed from.
    fn account_wasm_hash(env: Env) -> BytesN<32>;

    /// Extends the registry's instance TTL. Anyone may call it.
    fn extend_ttl(env: Env);
}
```

### Registry errors

| Code | Name | When |
|---|---|---|
| 1 | `AlreadyCreated` | `create_account` called twice for the same NFT |

### Registry events

| Topics | Data | When |
|---|---|---|
| `("created", nft: Address, token_id: u32)` | `account: Address` | An account was deployed |

## Account

```rust
/// A smart-contract account controlled by the current owner of one NFT.
pub trait HeybeAccount {
    /// Binds the account to an NFT. Called by the registry at deployment.
    fn __constructor(env: Env, nft: Address, token_id: u32);

    /// The NFT this account is bound to.
    fn token(env: Env) -> (Address, u32);

    /// Current owner, read live from nft.owner_of(token_id).
    fn owner(env: Env) -> Address;

    /// Increments on every execute, successful __check_auth and lock.
    /// Marketplaces compare it before and after a sale (see threat model).
    fn state(env: Env) -> u64;

    /// Ledger sequence until which the account is locked (0 = never locked).
    fn locked_until(env: Env) -> u32;

    /// Path A. Calls `func` on `target` with `args` as this account.
    /// Requires the current owner's auth.
    /// Errors: Locked, SelfOwned, SelfTransferBlocked.
    fn execute(env: Env, target: Address, func: Symbol, args: Vec<Val>) -> Val;

    /// Locks the account until `until_ledger`. While locked, execute and
    /// __check_auth fail. A lock can be extended but never shortened.
    /// Requires the current owner's auth.
    /// Errors: SelfOwned, InvalidLock.
    fn lock(env: Env, until_ledger: u32);

    /// Extends the account's instance TTL. Anyone may call it.
    fn extend_ttl(env: Env);
}

/// Path B (CAP-71). Implemented through soroban_sdk::auth::CustomAccountInterface.
impl CustomAccountInterface for Account {
    type Signature = ();            // the account verifies no signature itself
    type Error = AccountError;

    /// Accepts exactly one delegated signer: the current NFT owner.
    /// Then calls env.custom_account().delegate_auth(&owner).
    /// Errors: Locked, SelfOwned, InvalidDelegates.
    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signatures: (),
        auth_contexts: Vec<Context>,
    ) -> Result<(), AccountError>;
}
```

### Account errors

| Code | Name | When |
|---|---|---|
| 1 | `Locked` | The current ledger is before `locked_until` |
| 2 | `SelfOwned` | `owner_of` returns the account itself (the account would be unreachable) |
| 3 | `SelfTransferBlocked` | `execute` tries to transfer the bound NFT to the account itself |
| 4 | `InvalidDelegates` | Path B auth entry carries delegates other than exactly `[owner]` |
| 5 | `InvalidLock` | `until_ledger` is not after the current lock, is in the past, or is too far ahead |

### Account events

| Topics | Data | When |
|---|---|---|
| `("executed", target: Address, func: Symbol)` | `state: u64` | `execute` succeeded |
| `("locked",)` | `until_ledger: u32` | `lock` succeeded |

### Limits

- `MAX_LOCK_LEDGERS = 535_680`, about 31 days at 5-second ledgers. This keeps a lost lock from freezing an account for a long time.
- The self-transfer check covers the bound NFT's SEP-50 functions:
  - `transfer(from, to, token_id)`, checking argument index 1
  - `transfer_from(spender, from, to, token_id)`, checking argument index 2

## Storage keys

```rust
#[contracttype]
enum RegistryKey { AccountWasm, Created(Address, u32) }

#[contracttype]
enum AccountKey { Nft, TokenId, State, LockedUntil }
```
