//! Heybe test NFT.
//!
//! A plain OpenZeppelin SEP-50 NFT (sequential `u32` ids) where anyone can mint
//! a token to themselves. It exists only for tests and the testnet demo.

#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env, String};
use stellar_tokens::non_fungible::{
    burnable::NonFungibleBurnable, Base, Compose, NonFungibleToken,
};

#[contract]
pub struct HeybeTestNft;

#[contractimpl]
impl HeybeTestNft {
    pub fn __constructor(e: &Env, uri: String, name: String, symbol: String) {
        Base::set_metadata(e, uri, name, symbol);
    }

    /// Mints the next token id to `to`. Open to anyone; `to` must authorize.
    pub fn mint(e: &Env, to: Address) -> u32 {
        to.require_auth();
        <Self as NonFungibleToken>::ContractType::mint(e, &to)
    }
}

#[contractimpl(contracttrait)]
impl NonFungibleToken for HeybeTestNft {
    type ContractType = Compose<(Base,)>;
}

#[contractimpl(contracttrait)]
impl NonFungibleBurnable for HeybeTestNft {}
