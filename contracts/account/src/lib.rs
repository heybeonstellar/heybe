//! Heybe account: a token-bound account for Soroban.
//!
//! The account is bound to one NFT `(nft, token_id)`. It never stores an owner.
//! Every authorization reads `nft.owner_of(token_id)` and accepts only that
//! address, so control moves together with the NFT.
//!
//! Two ways to use it:
//! - Path A, [`HeybeAccount::execute`]: the owner signs a call that the account
//!   forwards to a target contract. Works with any wallet.
//! - Path B, `__check_auth`: the account is a CAP-71 custom account that
//!   delegates authentication to the current owner with `delegate_auth`.

#![no_std]

use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractevent, contractimpl, contracttype,
    crypto::Hash,
    symbol_short, vec, Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};

/// Longest lock an owner can set: about 31 days at 5-second ledgers.
pub const MAX_LOCK_LEDGERS: u32 = 535_680;

/// Instance TTL is extended once it drops below ~30 days, up to ~90 days.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_TTL_EXTEND_TO: u32 = 90 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccountError {
    /// The current ledger is before `locked_until`.
    Locked = 1,
    /// `owner_of` returned the account itself; nobody can authorize it.
    SelfOwned = 2,
    /// `execute` tried to transfer the bound NFT to the account itself.
    SelfTransferBlocked = 3,
    /// A Path B auth entry carried delegates other than exactly `[owner]`.
    InvalidDelegates = 4,
    /// The requested lock is in the past, shorter than the current one, or too long.
    InvalidLock = 5,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Nft,
    TokenId,
    State,
    LockedUntil,
}

/// Emitted after every successful `execute`.
#[contractevent(topics = ["executed"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Executed {
    #[topic]
    pub target: Address,
    #[topic]
    pub func: Symbol,
    pub state: u64,
}

/// Emitted after every successful `lock`.
#[contractevent(topics = ["locked"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockSet {
    pub until_ledger: u32,
}

#[contract]
pub struct HeybeAccount;

#[contractimpl]
impl HeybeAccount {
    /// Binds the account to an NFT. Called by the registry at deployment.
    pub fn __constructor(env: Env, nft: Address, token_id: u32) {
        let storage = env.storage().instance();
        storage.set(&DataKey::Nft, &nft);
        storage.set(&DataKey::TokenId, &token_id);
        storage.set(&DataKey::State, &0u64);
        storage.set(&DataKey::LockedUntil, &0u32);
        extend_instance(&env);
    }

    /// The NFT this account is bound to.
    pub fn token(env: Env) -> (Address, u32) {
        (bound_nft(&env), bound_token_id(&env))
    }

    /// Current owner, read live from `nft.owner_of(token_id)`.
    pub fn owner(env: Env) -> Address {
        current_owner(&env)
    }

    /// Increments on every `execute`, successful `__check_auth` and `lock`.
    pub fn state(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::State).unwrap_or(0)
    }

    /// Ledger sequence until which the account is locked (0 = never locked).
    pub fn locked_until(env: Env) -> u32 {
        read_locked_until(&env)
    }

    /// Path A. Calls `func(args)` on `target` as this account.
    ///
    /// Requires the current NFT owner's authorization. A target that calls
    /// `require_auth` on this account is satisfied automatically, because the
    /// account is its direct invoker.
    pub fn execute(
        env: Env,
        target: Address,
        func: Symbol,
        args: Vec<Val>,
    ) -> Result<Val, AccountError> {
        let owner = checked_owner(&env)?;
        ensure_unlocked(&env)?;
        owner.require_auth();

        if target == bound_nft(&env) && is_bound_self_transfer(&env, &func, &args) {
            return Err(AccountError::SelfTransferBlocked);
        }

        let state = bump_state(&env);
        extend_instance(&env);
        Executed {
            target: target.clone(),
            func: func.clone(),
            state,
        }
        .publish(&env);

        Ok(env.invoke_contract::<Val>(&target, &func, args))
    }

    /// Locks the account until `until_ledger`. While locked, `execute` and
    /// `__check_auth` fail. A lock can be extended but never shortened.
    pub fn lock(env: Env, until_ledger: u32) -> Result<(), AccountError> {
        let owner = checked_owner(&env)?;
        owner.require_auth();

        let now = env.ledger().sequence();
        let current = read_locked_until(&env);
        let max = now.saturating_add(MAX_LOCK_LEDGERS);
        if until_ledger <= now || until_ledger <= current || until_ledger > max {
            return Err(AccountError::InvalidLock);
        }

        env.storage()
            .instance()
            .set(&DataKey::LockedUntil, &until_ledger);
        bump_state(&env);
        extend_instance(&env);
        LockSet { until_ledger }.publish(&env);
        Ok(())
    }

    /// Extends the account's instance TTL. Anyone may call it.
    pub fn extend_ttl(env: Env) {
        extend_instance(&env);
    }
}

#[contractimpl]
impl CustomAccountInterface for HeybeAccount {
    /// The account verifies no signature itself; the owner signs as a delegate.
    type Signature = ();
    type Error = AccountError;

    /// Path B (CAP-71). Accepts exactly one delegated signer, the current NFT
    /// owner, and forwards the authorization to it.
    fn __check_auth(
        env: Env,
        _signature_payload: Hash<32>,
        _signatures: (),
        _auth_contexts: Vec<Context>,
    ) -> Result<(), AccountError> {
        let owner = checked_owner(&env)?;
        ensure_unlocked(&env)?;

        let delegates = env.custom_account().get_delegated_signers();
        if delegates.len() != 1 || delegates.get_unchecked(0) != owner {
            return Err(AccountError::InvalidDelegates);
        }

        bump_state(&env);
        extend_instance(&env);
        env.custom_account().delegate_auth(&owner);
        Ok(())
    }
}

fn bound_nft(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Nft).unwrap()
}

fn bound_token_id(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::TokenId).unwrap()
}

fn read_locked_until(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::LockedUntil)
        .unwrap_or(0)
}

fn current_owner(env: &Env) -> Address {
    let token_id = bound_token_id(env);
    env.invoke_contract::<Address>(
        &bound_nft(env),
        &symbol_short!("owner_of"),
        vec![env, token_id.into_val(env)],
    )
}

/// The current owner, unless the account owns its own NFT.
fn checked_owner(env: &Env) -> Result<Address, AccountError> {
    let owner = current_owner(env);
    if owner == env.current_contract_address() {
        return Err(AccountError::SelfOwned);
    }
    Ok(owner)
}

fn ensure_unlocked(env: &Env) -> Result<(), AccountError> {
    if env.ledger().sequence() < read_locked_until(env) {
        return Err(AccountError::Locked);
    }
    Ok(())
}

fn bump_state(env: &Env) -> u64 {
    let storage = env.storage().instance();
    let next = storage.get::<_, u64>(&DataKey::State).unwrap_or(0) + 1;
    storage.set(&DataKey::State, &next);
    next
}

fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

/// True when `func(args)` on the bound NFT contract would send the bound token
/// to this account (SEP-50 `transfer` / `transfer_from`).
fn is_bound_self_transfer(env: &Env, func: &Symbol, args: &Vec<Val>) -> bool {
    let (to_index, id_index) = if *func == symbol_short!("transfer") {
        (1, 2)
    } else if *func == Symbol::new(env, "transfer_from") {
        (2, 3)
    } else {
        return false;
    };

    let to = args
        .get(to_index)
        .and_then(|v| Address::try_from_val(env, &v).ok());
    let id = args
        .get(id_index)
        .and_then(|v| u32::try_from_val(env, &v).ok());

    to == Some(env.current_contract_address()) && id == Some(bound_token_id(env))
}

#[cfg(test)]
mod test;
