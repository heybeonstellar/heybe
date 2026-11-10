//! Heybe registry.
//!
//! Computes one deterministic account address per NFT `(nft, token_id)` and
//! deploys the Heybe account contract there. Deployment is permissionless:
//! creating an account grants no control, because control always comes from
//! the NFT contract's `owner_of`.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, xdr::ToXdr, Address,
    BytesN, ContractExecutable, Env,
};

const DAY_IN_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
const TTL_EXTEND_TO: u32 = 90 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// `create_account` was already called for this NFT.
    AlreadyCreated = 1,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    AccountWasm,
    Created(Address, u32),
}

/// Emitted when an account is deployed.
#[contractevent(topics = ["created"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountCreated {
    #[topic]
    pub nft: Address,
    #[topic]
    pub token_id: u32,
    pub account: Address,
}

#[contract]
pub struct HeybeRegistry;

#[contractimpl]
impl HeybeRegistry {
    /// Stores the wasm hash every account is deployed from.
    pub fn __constructor(env: Env, account_wasm_hash: BytesN<32>) {
        env.storage()
            .instance()
            .set(&DataKey::AccountWasm, &account_wasm_hash);
        extend_instance(&env);
    }

    /// Deterministic account address for an NFT. Works before the account exists.
    pub fn account_of(env: Env, nft: Address, token_id: u32) -> Address {
        env.deployer()
            .with_current_contract(salt(&env, &nft, token_id))
            .deployed_address()
    }

    /// Deploys the account for an NFT and returns its address.
    pub fn create_account(env: Env, nft: Address, token_id: u32) -> Result<Address, RegistryError> {
        let key = DataKey::Created(nft.clone(), token_id);
        let storage = env.storage().persistent();
        if storage.has(&key) {
            return Err(RegistryError::AlreadyCreated);
        }

        let wasm_hash: BytesN<32> = env.storage().instance().get(&DataKey::AccountWasm).unwrap();
        let account = env
            .deployer()
            .with_current_contract(salt(&env, &nft, token_id))
            .deploy_contract(ContractExecutable::Wasm(wasm_hash), (nft.clone(), token_id));

        storage.set(&key, &account);
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        extend_instance(&env);

        AccountCreated {
            nft,
            token_id,
            account: account.clone(),
        }
        .publish(&env);
        Ok(account)
    }

    /// True once `create_account` has succeeded for this NFT.
    pub fn is_created(env: Env, nft: Address, token_id: u32) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Created(nft, token_id))
    }

    /// The wasm hash every account is deployed from.
    pub fn account_wasm_hash(env: Env) -> BytesN<32> {
        env.storage().instance().get(&DataKey::AccountWasm).unwrap()
    }

    /// Extends the registry's instance TTL. Anyone may call it.
    pub fn extend_ttl(env: Env) {
        extend_instance(&env);
    }
}

/// `sha256( xdr(nft) || be_bytes(token_id) )`
fn salt(env: &Env, nft: &Address, token_id: u32) -> BytesN<32> {
    let mut preimage = nft.clone().to_xdr(env);
    preimage.extend_from_array(&token_id.to_be_bytes());
    env.crypto().sha256(&preimage).to_bytes()
}

fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

#[cfg(test)]
mod test;
