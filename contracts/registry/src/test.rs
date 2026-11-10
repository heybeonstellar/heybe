#![cfg(test)]
//! End-to-end tests: the registry deploys the real account wasm.
//!
//! Build the wasm first: `stellar contract build` (from `contracts/`).

extern crate std;

use crate::{HeybeRegistry, HeybeRegistryClient, RegistryError};
use heybe_account::HeybeAccountClient;
use heybe_test_nft::{HeybeTestNft, HeybeTestNftClient};
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl,
    crypto::Hash,
    symbol_short,
    testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    xdr::{
        InvokeContractArgs, ScAddress, ScVal, SorobanAddressCredentials,
        SorobanAddressCredentialsWithDelegates, SorobanAuthorizationEntry,
        SorobanAuthorizedFunction, SorobanAuthorizedInvocation, SorobanCredentials,
        SorobanDelegateSignature, StringM, VecM,
    },
    Address, Env, IntoVal, String, Val, Vec,
};

const ACCOUNT_WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/heybe_account.wasm");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TestError {
    Rejected = 1,
}

/// Owner stand-in for Path B: approves every authorization.
#[contract]
pub struct ApproveAll;

#[contractimpl]
impl CustomAccountInterface for ApproveAll {
    type Signature = ();
    type Error = TestError;
    fn __check_auth(_e: Env, _p: Hash<32>, _s: (), _c: Vec<Context>) -> Result<(), TestError> {
        Ok(())
    }
}

#[contract]
pub struct Protected;

#[contractimpl]
impl Protected {
    pub fn protected(_env: Env, who: Address) {
        who.require_auth();
    }
}

struct World<'a> {
    env: Env,
    registry: HeybeRegistryClient<'a>,
    nft: HeybeTestNftClient<'a>,
    xlm: TokenClient<'a>,
    xlm_admin: StellarAssetClient<'a>,
}

fn world<'a>() -> World<'a> {
    let env = Env::default();
    let wasm_hash = env.deployer().upload_contract_wasm(ACCOUNT_WASM);
    let registry_id = env.register(HeybeRegistry, (wasm_hash,));
    let nft_id = env.register(
        HeybeTestNft,
        (
            String::from_str(&env, "https://heybe.example/"),
            String::from_str(&env, "Heybe Test"),
            String::from_str(&env, "HBT"),
        ),
    );
    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    World {
        registry: HeybeRegistryClient::new(&env, &registry_id),
        nft: HeybeTestNftClient::new(&env, &nft_id),
        xlm: TokenClient::new(&env, &sac.address()),
        xlm_admin: StellarAssetClient::new(&env, &sac.address()),
        env,
    }
}

fn pay_args(env: &Env, from: &Address, to: &Address, amount: i128) -> Vec<Val> {
    (from.clone(), to.clone(), amount).into_val(env)
}

fn auth_execute_as(env: &Env, who: &Address, account: &Address, target: &Address, args: &Vec<Val>) {
    let call_args: Vec<Val> =
        (target.clone(), symbol_short!("transfer"), args.clone()).into_val(env);
    env.mock_auths(&[MockAuth {
        address: who,
        invoke: &MockAuthInvoke {
            contract: account,
            fn_name: "execute",
            args: call_args,
            sub_invokes: &[],
        },
    }]);
}

#[test]
fn account_address_is_deterministic() {
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let ali = Address::generate(env);
    let t1 = w.nft.mint(&ali);
    let t2 = w.nft.mint(&ali);

    let a1 = w.registry.account_of(&w.nft.address, &t1);
    assert_eq!(a1, w.registry.account_of(&w.nft.address, &t1));
    assert_ne!(a1, w.registry.account_of(&w.nft.address, &t2));

    // A different collection with the same token id gets a different account.
    let other_nft = Address::generate(env);
    assert_ne!(a1, w.registry.account_of(&other_nft, &t1));

    assert!(!w.registry.is_created(&w.nft.address, &t1));
    let created = w.registry.create_account(&w.nft.address, &t1);
    assert_eq!(created, a1);
    assert!(w.registry.is_created(&w.nft.address, &t1));

    let account = HeybeAccountClient::new(env, &created);
    assert_eq!(account.token(), (w.nft.address.clone(), t1));
    assert_eq!(account.owner(), ali);
}

#[test]
fn create_is_permissionless_and_once_per_nft() {
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let ali = Address::generate(env);
    let t = w.nft.mint(&ali);

    // No auth set at all: anyone can create.
    env.set_auths(&[]);
    w.registry.create_account(&w.nft.address, &t);
    assert_eq!(
        w.registry.try_create_account(&w.nft.address, &t),
        Err(Ok(RegistryError::AlreadyCreated))
    );
}

#[test]
fn create_emits_created_event() {
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let ali = Address::generate(env);
    let t = w.nft.mint(&ali);
    let account = w.registry.create_account(&w.nft.address, &t);

    let events = env.events().all();
    let ev = events.events().last().expect("an event");
    let body = std::format!("{:?}", ev);
    assert!(body.contains("created"), "unexpected event: {body}");
    assert!(w.registry.is_created(&w.nft.address, &t));
    assert_eq!(account, w.registry.account_of(&w.nft.address, &t));
}

#[test]
fn funds_sent_before_deployment_arrive() {
    // Spike: the SAC accepts a transfer to the account address before the
    // account contract exists, and the deployed account can then spend it.
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let ali = Address::generate(env);
    let bob = Address::generate(env);
    let t = w.nft.mint(&ali);

    let future = w.registry.account_of(&w.nft.address, &t);
    w.xlm_admin.mint(&ali, &1_000);
    w.xlm.transfer(&ali, &future, &250);
    assert_eq!(w.xlm.balance(&future), 250);

    let account = w.registry.create_account(&w.nft.address, &t);
    assert_eq!(account, future);

    let args = pay_args(env, &account, &bob, 200);
    auth_execute_as(env, &ali, &account, &w.xlm.address, &args);
    HeybeAccountClient::new(env, &account).execute(
        &w.xlm.address,
        &symbol_short!("transfer"),
        &args,
    );
    assert_eq!(w.xlm.balance(&account), 50);
    assert_eq!(w.xlm.balance(&bob), 200);
}

#[test]
fn end_to_end_control_follows_nft() {
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let ali = Address::generate(env);
    let ayse = Address::generate(env);
    let bob = Address::generate(env);
    let t = w.nft.mint(&ali);
    let account_id = w.registry.create_account(&w.nft.address, &t);
    let account = HeybeAccountClient::new(env, &account_id);
    w.xlm_admin.mint(&account_id, &1_000);

    // 1. Ali pays from the account.
    let args = pay_args(env, &account_id, &bob, 100);
    auth_execute_as(env, &ali, &account_id, &w.xlm.address, &args);
    account.execute(&w.xlm.address, &symbol_short!("transfer"), &args);
    assert_eq!(w.xlm.balance(&account_id), 900);

    // 2. The NFT moves to Ayşe.
    env.mock_all_auths();
    w.nft.transfer(&ali, &ayse, &t);
    assert_eq!(account.owner(), ayse);

    // 3. Ali is rejected.
    auth_execute_as(env, &ali, &account_id, &w.xlm.address, &args);
    assert!(account
        .try_execute(&w.xlm.address, &symbol_short!("transfer"), &args)
        .is_err());
    assert_eq!(w.xlm.balance(&account_id), 900);

    // 4. Ayşe pays from the same account.
    auth_execute_as(env, &ayse, &account_id, &w.xlm.address, &args);
    account.execute(&w.xlm.address, &symbol_short!("transfer"), &args);
    assert_eq!(w.xlm.balance(&account_id), 800);
    assert_eq!(w.xlm.balance(&bob), 200);
}

#[test]
fn path_b_with_deployed_wasm_account() {
    let w = world();
    let env = &w.env;
    env.mock_all_auths();
    let owner = env.register(ApproveAll, ());
    let t = w.nft.mint(&owner);
    let account_id = w.registry.create_account(&w.nft.address, &t);
    let protected = ProtectedClient::new(env, &env.register(Protected, ()));

    let account_sc: ScAddress = account_id.clone().into();
    env.set_auths(&[SorobanAuthorizationEntry {
        credentials: SorobanCredentials::AddressWithDelegates(
            SorobanAddressCredentialsWithDelegates {
                address_credentials: SorobanAddressCredentials {
                    address: account_sc.clone(),
                    nonce: 1,
                    signature_expiration_ledger: env.ledger().sequence() + 100,
                    signature: ScVal::Void,
                },
                delegates: std::vec![SorobanDelegateSignature {
                    address: owner.clone().into(),
                    signature: ScVal::Void,
                    nested_delegates: VecM::default(),
                }]
                .try_into()
                .unwrap(),
            },
        ),
        root_invocation: SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: protected.address.clone().into(),
                function_name: StringM::try_from("protected").unwrap().into(),
                args: std::vec![ScVal::Address(account_sc)].try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        },
    }]);
    protected.protected(&account_id);
    assert_eq!(HeybeAccountClient::new(env, &account_id).state(), 1);
}

#[test]
fn exposes_account_wasm_hash() {
    let w = world();
    let expected = w.env.deployer().upload_contract_wasm(ACCOUNT_WASM);
    assert_eq!(w.registry.account_wasm_hash(), expected);
}
