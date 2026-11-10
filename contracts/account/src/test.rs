#![cfg(test)]
extern crate std;

use crate::{AccountError, HeybeAccount, HeybeAccountClient, MAX_LOCK_LEDGERS};
use heybe_test_nft::{HeybeTestNft, HeybeTestNftClient};
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl,
    crypto::Hash,
    symbol_short,
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    xdr::{
        InvokeContractArgs, ScAddress, ScVal, SorobanAddressCredentials,
        SorobanAddressCredentialsWithDelegates, SorobanAuthorizationEntry,
        SorobanAuthorizedFunction, SorobanAuthorizedInvocation, SorobanCredentials,
        SorobanDelegateSignature, StringM, VecM,
    },
    Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec,
};

// ---------------------------------------------------------------------------
// Test-only contracts
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TestError {
    Rejected = 1,
}

/// A custom account that approves every authorization. Stands in for an
/// owner wallet so Path B can be tested with real (non-mocked) auth entries.
#[contract]
pub struct ApproveAll;

#[contractimpl]
impl CustomAccountInterface for ApproveAll {
    type Signature = ();
    type Error = TestError;
    fn __check_auth(
        _env: Env,
        _payload: Hash<32>,
        _sigs: (),
        _ctx: Vec<Context>,
    ) -> Result<(), TestError> {
        Ok(())
    }
}

/// A contract with one function that requires `who` to authorize.
#[contract]
pub struct Protected;

#[contractimpl]
impl Protected {
    pub fn protected(_env: Env, who: Address) {
        who.require_auth();
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct Setup<'a> {
    env: Env,
    nft: HeybeTestNftClient<'a>,
    account: HeybeAccountClient<'a>,
    token_id: u32,
}

/// Registers a test NFT, mints token to `owner`, and registers the account
/// bound to that token (natively, without the registry).
fn setup_with_owner<'a>(env: &Env, owner: &Address) -> Setup<'a> {
    let nft_id = env.register(
        HeybeTestNft,
        (
            String::from_str(env, "https://heybe.example/"),
            String::from_str(env, "Heybe Test"),
            String::from_str(env, "HBT"),
        ),
    );
    let nft = HeybeTestNftClient::new(env, &nft_id);

    env.mock_all_auths();
    let token_id = nft.mint(owner);

    let account_id = env.register(HeybeAccount, (nft_id.clone(), token_id));
    let account = HeybeAccountClient::new(env, &account_id);
    Setup {
        env: env.clone(),
        nft,
        account,
        token_id,
    }
}

fn xlm<'a>(env: &Env) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let admin = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(admin);
    (
        sac.address(),
        TokenClient::new(env, &sac.address()),
        StellarAssetClient::new(env, &sac.address()),
    )
}

/// The account's own error from a `try_*` call, if that is what failed.
fn account_err<T, C>(r: Result<T, Result<AccountError, C>>) -> Option<AccountError> {
    match r {
        Err(Ok(e)) => Some(e),
        _ => None,
    }
}

fn transfer_args(env: &Env, from: &Address, to: &Address, amount: i128) -> Vec<Val> {
    (from.clone(), to.clone(), amount).into_val(env)
}

/// Authorizes exactly `owner -> account.execute(target, func, args)` and nothing else.
fn auth_execute_as(s: &Setup, who: &Address, target: &Address, func: &str, args: &Vec<Val>) {
    let call_args: Vec<Val> =
        (target.clone(), Symbol::new(&s.env, func), args.clone()).into_val(&s.env);
    s.env.mock_auths(&[MockAuth {
        address: who,
        invoke: &MockAuthInvoke {
            contract: &s.account.address,
            fn_name: "execute",
            args: call_args,
            sub_invokes: &[],
        },
    }]);
}

/// Builds a CAP-71 auth entry for `account` that attaches `delegates` and
/// authorizes `protected.protected(account)`.
fn path_b_entry(
    env: &Env,
    account: &Address,
    protected: &Address,
    delegates: &[Address],
    nonce: i64,
) -> SorobanAuthorizationEntry {
    let account_sc: ScAddress = account.clone().into();
    let mut delegate_list: std::vec::Vec<SorobanDelegateSignature> = delegates
        .iter()
        .map(|d| SorobanDelegateSignature {
            address: d.clone().into(),
            signature: ScVal::Void,
            nested_delegates: VecM::default(),
        })
        .collect();
    delegate_list.sort_by(|a, b| a.address.cmp(&b.address));

    SorobanAuthorizationEntry {
        credentials: SorobanCredentials::AddressWithDelegates(
            SorobanAddressCredentialsWithDelegates {
                address_credentials: SorobanAddressCredentials {
                    address: account_sc.clone(),
                    nonce,
                    signature_expiration_ledger: env.ledger().sequence() + 100,
                    signature: ScVal::Void,
                },
                delegates: delegate_list.try_into().unwrap(),
            },
        ),
        root_invocation: SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: protected.clone().into(),
                function_name: StringM::try_from("protected").unwrap().into(),
                args: std::vec![ScVal::Address(account_sc)].try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        },
    }
}

// ---------------------------------------------------------------------------
// Basics
// ---------------------------------------------------------------------------

#[test]
fn binds_to_nft_and_reads_owner_live() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let ayse = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    assert_eq!(s.account.token(), (s.nft.address.clone(), s.token_id));
    assert_eq!(s.account.owner(), ali);
    assert_eq!(s.account.state(), 0);
    assert_eq!(s.account.locked_until(), 0);

    s.nft.transfer(&ali, &ayse, &s.token_id);
    assert_eq!(s.account.owner(), ayse);
}

// ---------------------------------------------------------------------------
// Path A: execute
// ---------------------------------------------------------------------------

#[test]
fn owner_pays_from_account_with_execute() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let bob = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);
    let (xlm_id, xlm, xlm_admin) = xlm(&env);
    xlm_admin.mint(&s.account.address, &1_000);

    let args = transfer_args(&env, &s.account.address, &bob, 100);
    auth_execute_as(&s, &ali, &xlm_id, "transfer", &args);
    s.account
        .execute(&xlm_id, &symbol_short!("transfer"), &args);
    // Only Ali's signature was needed; the account authorized as direct invoker.
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, ali);

    assert_eq!(xlm.balance(&s.account.address), 900);
    assert_eq!(xlm.balance(&bob), 100);
    assert_eq!(s.account.state(), 1);
}

#[test]
fn control_moves_with_the_nft() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let ayse = Address::generate(&env);
    let bob = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);
    let (xlm_id, xlm, xlm_admin) = xlm(&env);
    xlm_admin.mint(&s.account.address, &1_000);

    env.mock_all_auths();
    s.nft.transfer(&ali, &ayse, &s.token_id);

    // Ali signs, but Ali is no longer the owner: rejected.
    let args = transfer_args(&env, &s.account.address, &bob, 100);
    auth_execute_as(&s, &ali, &xlm_id, "transfer", &args);
    assert!(s
        .account
        .try_execute(&xlm_id, &symbol_short!("transfer"), &args)
        .is_err());
    assert_eq!(xlm.balance(&s.account.address), 1_000);

    // Ayşe signs the same call: accepted.
    auth_execute_as(&s, &ayse, &xlm_id, "transfer", &args);
    s.account
        .execute(&xlm_id, &symbol_short!("transfer"), &args);
    assert_eq!(xlm.balance(&s.account.address), 900);
    assert_eq!(xlm.balance(&bob), 100);
}

#[test]
fn stranger_cannot_execute() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let mallory = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);
    let (xlm_id, xlm, xlm_admin) = xlm(&env);
    xlm_admin.mint(&s.account.address, &500);

    let args = transfer_args(&env, &s.account.address, &mallory, 500);
    auth_execute_as(&s, &mallory, &xlm_id, "transfer", &args);
    assert!(s
        .account
        .try_execute(&xlm_id, &symbol_short!("transfer"), &args)
        .is_err());
    assert_eq!(xlm.balance(&s.account.address), 500);
}

#[test]
fn account_holds_and_sends_other_nfts() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let bob = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    // A second token of the same collection, held by the account.
    env.mock_all_auths();
    let other = s.nft.mint(&s.account.address);
    assert_eq!(s.nft.owner_of(&other), s.account.address);

    let args: Vec<Val> = (s.account.address.clone(), bob.clone(), other).into_val(&env);
    auth_execute_as(&s, &ali, &s.nft.address, "transfer", &args);
    s.account
        .execute(&s.nft.address, &symbol_short!("transfer"), &args);
    assert_eq!(s.nft.owner_of(&other), bob);
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

#[test]
fn blocks_sending_the_bound_nft_to_itself() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    env.mock_all_auths();
    let args: Vec<Val> = (ali.clone(), s.account.address.clone(), s.token_id).into_val(&env);
    let res = s
        .account
        .try_execute(&s.nft.address, &symbol_short!("transfer"), &args);
    assert_eq!(account_err(res), Some(AccountError::SelfTransferBlocked));
    assert_eq!(s.nft.owner_of(&s.token_id), ali);
}

#[test]
fn self_owned_account_reports_self_owned() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    // The owner bypasses Heybe and sends the NFT to the account directly.
    env.mock_all_auths();
    s.nft.transfer(&ali, &s.account.address, &s.token_id);

    let (xlm_id, _, _) = xlm(&env);
    let args = transfer_args(&env, &s.account.address, &ali, 1);
    let res = s
        .account
        .try_execute(&xlm_id, &symbol_short!("transfer"), &args);
    assert_eq!(account_err(res), Some(AccountError::SelfOwned));
    assert_eq!(s.account.try_lock(&100), Err(Ok(AccountError::SelfOwned)));
}

#[test]
fn burned_nft_freezes_the_account() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);
    let (xlm_id, _, xlm_admin) = xlm(&env);
    xlm_admin.mint(&s.account.address, &10);

    env.mock_all_auths();
    s.nft.burn(&ali, &s.token_id);

    let args = transfer_args(&env, &s.account.address, &ali, 10);
    assert!(s
        .account
        .try_execute(&xlm_id, &symbol_short!("transfer"), &args)
        .is_err());
}

// ---------------------------------------------------------------------------
// Lock
// ---------------------------------------------------------------------------

#[test]
fn lock_blocks_execute_until_it_expires() {
    let env = Env::default();
    env.ledger().set_sequence_number(1_000);
    let ali = Address::generate(&env);
    let bob = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);
    let (xlm_id, xlm, xlm_admin) = xlm(&env);
    xlm_admin.mint(&s.account.address, &100);

    env.mock_all_auths();
    s.account.lock(&1_100);
    assert_eq!(s.account.locked_until(), 1_100);
    assert_eq!(s.account.state(), 1);

    let args = transfer_args(&env, &s.account.address, &bob, 50);
    let res = s
        .account
        .try_execute(&xlm_id, &symbol_short!("transfer"), &args);
    assert_eq!(account_err(res), Some(AccountError::Locked));

    // Assets can still arrive while locked.
    xlm_admin.mint(&s.account.address, &5);
    assert_eq!(xlm.balance(&s.account.address), 105);

    env.ledger().set_sequence_number(1_100);
    s.account
        .execute(&xlm_id, &symbol_short!("transfer"), &args);
    assert_eq!(xlm.balance(&bob), 50);
}

#[test]
fn lock_can_be_extended_but_not_shortened() {
    let env = Env::default();
    env.ledger().set_sequence_number(1_000);
    let ali = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    env.mock_all_auths();
    s.account.lock(&2_000);
    assert_eq!(
        s.account.try_lock(&1_500),
        Err(Ok(AccountError::InvalidLock))
    );
    assert_eq!(
        s.account.try_lock(&2_000),
        Err(Ok(AccountError::InvalidLock))
    );
    s.account.lock(&3_000);
    assert_eq!(s.account.locked_until(), 3_000);
}

#[test]
fn lock_rejects_past_and_too_long() {
    let env = Env::default();
    env.ledger().set_sequence_number(1_000);
    let ali = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    env.mock_all_auths();
    assert_eq!(s.account.try_lock(&999), Err(Ok(AccountError::InvalidLock)));
    assert_eq!(
        s.account.try_lock(&1_000),
        Err(Ok(AccountError::InvalidLock))
    );
    assert_eq!(
        s.account.try_lock(&(1_000 + MAX_LOCK_LEDGERS + 1)),
        Err(Ok(AccountError::InvalidLock))
    );
    s.account.lock(&(1_000 + MAX_LOCK_LEDGERS));
}

#[test]
fn only_owner_can_lock() {
    let env = Env::default();
    let ali = Address::generate(&env);
    let mallory = Address::generate(&env);
    let s = setup_with_owner(&env, &ali);

    env.mock_auths(&[MockAuth {
        address: &mallory,
        invoke: &MockAuthInvoke {
            contract: &s.account.address,
            fn_name: "lock",
            args: (100u32,).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(s.account.try_lock(&100).is_err());
    assert_eq!(s.account.locked_until(), 0);
}

// ---------------------------------------------------------------------------
// Path B: CAP-71 delegation with real auth entries (no mocks)
// ---------------------------------------------------------------------------

struct PathB<'a> {
    s: Setup<'a>,
    owner: Address,
    protected: ProtectedClient<'a>,
}

fn setup_path_b<'a>(env: &Env) -> PathB<'a> {
    let owner = env.register(ApproveAll, ());
    let s = setup_with_owner(env, &owner);
    let protected_id = env.register(Protected, ());
    PathB {
        s,
        owner,
        protected: ProtectedClient::new(env, &protected_id),
    }
}

#[test]
fn path_b_owner_delegate_authorizes() {
    let env = Env::default();
    let b = setup_path_b(&env);

    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        core::slice::from_ref(&b.owner),
        1,
    )]);
    b.protected.protected(&b.s.account.address);
    // The call was authorized by the account (one auth, made by the account).
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, b.s.account.address);

    // __check_auth ran and its storage write (state bump) persisted.
    assert_eq!(b.s.account.state(), 1);
}

#[test]
fn path_b_rejects_a_non_owner_delegate() {
    let env = Env::default();
    let b = setup_path_b(&env);
    let stranger = env.register(ApproveAll, ());

    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        &[stranger],
        1,
    )]);
    assert!(b.protected.try_protected(&b.s.account.address).is_err());
    assert_eq!(b.s.account.state(), 0);
}

#[test]
fn path_b_rejects_extra_delegates() {
    let env = Env::default();
    let b = setup_path_b(&env);
    let stranger = env.register(ApproveAll, ());

    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        &[b.owner.clone(), stranger],
        1,
    )]);
    assert!(b.protected.try_protected(&b.s.account.address).is_err());
}

#[test]
fn path_b_rejects_no_delegates() {
    let env = Env::default();
    let b = setup_path_b(&env);

    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        &[],
        1,
    )]);
    assert!(b.protected.try_protected(&b.s.account.address).is_err());
}

#[test]
fn path_b_follows_the_nft() {
    let env = Env::default();
    let b = setup_path_b(&env);
    let new_owner = env.register(ApproveAll, ());

    env.mock_all_auths();
    b.s.nft.transfer(&b.owner, &new_owner, &b.s.token_id);

    // Old owner as delegate: rejected.
    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        core::slice::from_ref(&b.owner),
        1,
    )]);
    assert!(b.protected.try_protected(&b.s.account.address).is_err());

    // New owner as delegate: accepted.
    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        &[new_owner],
        2,
    )]);
    b.protected.protected(&b.s.account.address);
}

#[test]
fn path_b_respects_lock() {
    let env = Env::default();
    env.ledger().set_sequence_number(1_000);
    let b = setup_path_b(&env);

    env.mock_all_auths();
    b.s.account.lock(&1_100);

    env.set_auths(&[path_b_entry(
        &env,
        &b.s.account.address,
        &b.protected.address,
        core::slice::from_ref(&b.owner),
        1,
    )]);
    assert!(b.protected.try_protected(&b.s.account.address).is_err());
}

#[test]
fn check_auth_error_codes() {
    // Calling __check_auth directly surfaces the account's own error codes for
    // the checks that run before delegation. (InvalidDelegates needs a real
    // CAP-71 auth entry and is covered by the path_b_* tests above.)
    let env = Env::default();
    env.ledger().set_sequence_number(1_000);
    let b = setup_path_b(&env);
    let payload = BytesN::from_array(&env, &[7u8; 32]);
    let ctx: Vec<Context> = Vec::new(&env);

    env.mock_all_auths();
    b.s.account.lock(&1_100);
    let res = env.try_invoke_contract_check_auth::<AccountError>(
        &b.s.account.address,
        &payload,
        ().into_val(&env),
        &ctx,
    );
    assert_eq!(res, Err(Ok(AccountError::Locked)));

    // Self-owned: the owner sends the bound NFT to the account directly.
    b.s.nft
        .transfer(&b.owner, &b.s.account.address, &b.s.token_id);
    let res = env.try_invoke_contract_check_auth::<AccountError>(
        &b.s.account.address,
        &payload,
        ().into_val(&env),
        &ctx,
    );
    assert_eq!(res, Err(Ok(AccountError::SelfOwned)));
}
