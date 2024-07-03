#![cfg(test)]

use super::*;
use crate::constants::DEFAULT_MEMO;
use crate::memo::generate_next_memo;
use crate::swap_router::swap_router;
use soroban_sdk::testutils::arbitrary::std;
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, MockAuth, MockAuthInvoke,
};
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, String, Symbol, Vec};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> SorobanTokenClient<'a> {
    SorobanTokenClient::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn deploy_liqpool_router_contract<'a>(e: &Env) -> swap_router::Client {
    swap_router::Client::new(e, &e.register_contract_wasm(None, swap_router::WASM))
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../../wasm/soroban_token_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

fn install_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../../wasm/soroban_liquidity_pool_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

fn install_stableswap_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../../wasm/soroban_liquidity_pool_stableswap_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn deploy_plane_contract<'a>(e: &Env) -> Address {
    soroban_sdk::contractimport!(file = "../../wasm/soroban_liquidity_pool_plane_contract.wasm");
    Client::new(e, &e.register_contract_wasm(None, WASM)).address
}

mod swap_calculator {
    soroban_sdk::contractimport!(
        file = "../../wasm/soroban_liquidity_pool_swap_router_contract.wasm"
    );
}

fn deploy_swap_calculator_contract<'a>(e: &Env) -> swap_calculator::Client {
    swap_calculator::Client::new(e, &e.register_contract_wasm(None, swap_calculator::WASM))
}

fn deploy_swap_pool<'a>(e: &Env) -> PoolContractClient<'a> {
    let pool = PoolContractClient::new(e, &e.register_contract(None, PoolContract {}));
    pool
}

#[test]
fn test_memo() {
    let e = Env::default();
    e.budget().reset_unlimited();
    let swap_pool = deploy_swap_pool(&e);
    let expected_memo = [
        "0000000000000000000000000000",
        "000000000000000000000000003e",
        "000000000000000000000000006s",
        "000000000000000000000000009G",
        "00000000000000000000000000cU",
    ];
    let iterations = 1000;
    let step = iterations / expected_memo.len();
    for i in 0..iterations {
        let user = Address::generate(&e);
        let token = Address::generate(&e);
        let memo = swap_pool.generate_user_memo(&user, &token);
        // println!("{:?}", memo.to_string());
        if i % step == 0 {
            assert_eq!(memo, String::from_str(&e, expected_memo[i / step]))
        }
    }
}

#[test]
fn test_memo_generation() {
    // memo should be unique for every user & token pair but should not change for the same pair
    let e = Env::default();
    e.budget().reset_unlimited();
    let swap_pool = deploy_swap_pool(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let token1 = Address::generate(&e);
    let token2 = Address::generate(&e);

    assert_eq!(swap_pool.has_user_memo(&user1, &token1), false);
    assert_eq!(swap_pool.has_user_memo(&user1, &token2), false);
    assert_eq!(swap_pool.has_user_memo(&user2, &token1), false);

    let memo1 = swap_pool.generate_user_memo(&user1, &token1);
    assert_eq!(swap_pool.has_user_memo(&user1, &token1), true);

    let memo2 = swap_pool.generate_user_memo(&user1, &token2);
    let memo3 = swap_pool.generate_user_memo(&user2, &token1);

    assert_ne!(memo1, memo2);
    assert_ne!(memo1, memo3);
    assert_ne!(memo2, memo3);

    assert_eq!(swap_pool.has_user_memo(&user1, &token2), true);
    assert_eq!(swap_pool.has_user_memo(&user2, &token1), true);

    // duplicate memo should not be created even if generator called twice
    assert_eq!(memo1, swap_pool.generate_user_memo(&user1, &token1));

    assert_eq!(memo1, swap_pool.get_user_memo(&user1, &token1));
}

#[should_panic(expected = "Error(Contract, #502)")]
#[test]
fn test_get_memo_if_not_exists() {
    // memo should be unique for every user & token pair but should not change for the same pair
    let e = Env::default();
    e.budget().reset_unlimited();
    let swap_pool = deploy_swap_pool(&e);
    let user = Address::generate(&e);
    let token = Address::generate(&e);
    swap_pool.get_user_memo(&user, &token);
}

#[test]
fn test_memo_light() {
    let e = Env::default();
    let expected_memo = [
        "0000000000000000000000000000",
        "000000000000000000000001lUUE",
        "000000000000000000000002HPPi",
        "0000000000000000000000043KJW",
        "000000000000000000000005pFEA",
    ];
    let iterations = 100_000_000_u64;
    let step = iterations / expected_memo.len() as u64;
    let mut default_memo = [0u8; 28];
    for i in 0..DEFAULT_MEMO.len() {
        default_memo[i] = DEFAULT_MEMO.as_bytes()[i];
    }
    let mut memo = default_memo;
    for i in 0..iterations {
        if i % step == 0 {
            assert_eq!(
                String::from_bytes(&e, &memo),
                String::from_str(&e, expected_memo[(i / step) as usize])
            )
        }
        memo = generate_next_memo(&memo);
    }
}

#[test]
fn test_chained_swap() {
    let e = Env::default();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let proxy_wallet = Address::generate(&e);
    let operator = Address::generate(&e);
    let destination = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token1_admin = SorobanTokenAdminClient::new(&e, &tokens[0]);
    let token2_admin = SorobanTokenAdminClient::new(&e, &tokens[1]);
    let token3_admin = SorobanTokenAdminClient::new(&e, &tokens[2]);

    let tokens1 = Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]);
    let tokens2 = Vec::from_array(&e, [tokens[1].clone(), tokens[2].clone()]);

    // init swap router with all it's complexity
    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = deploy_plane_contract(&e);
    let swap_router = deploy_swap_calculator_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.mock_all_auths().set_pools_plane(&admin, &plane);
    let router = deploy_liqpool_router_contract(&e);
    router.mock_all_auths().init_admin(&admin);
    router.mock_all_auths().set_pool_hash(&pool_hash);
    router
        .mock_all_auths()
        .set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.mock_all_auths().set_token_hash(&token_hash);
    router.mock_all_auths().set_reward_token(&token1.address);
    router.mock_all_auths().set_pools_plane(&admin, &plane);
    router
        .mock_all_auths()
        .set_swap_router(&admin, &swap_router.address);

    // init pools & deposit
    let (pool_index1, _pool_address1) = router
        .mock_all_auths()
        .init_standard_pool(&admin, &tokens1, &30);
    let (pool_index2, _pool_address2) = router
        .mock_all_auths()
        .init_standard_pool(&admin, &tokens2, &30);
    token1_admin.mock_all_auths().mint(&admin, &10000);
    token2_admin.mock_all_auths().mint(&admin, &20000);
    token3_admin.mock_all_auths().mint(&admin, &10000);
    router.mock_all_auths().deposit(
        &admin,
        &tokens1,
        &pool_index1,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );
    router.mock_all_auths().deposit(
        &admin,
        &tokens2,
        &pool_index2,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );

    // init current contract
    let swap_pool = deploy_swap_pool(&e);
    swap_pool.mock_all_auths().set_admin(&admin);
    swap_pool.mock_all_auths().set_operator(&operator);
    swap_pool.mock_all_auths().set_swap_router(&router.address);
    swap_pool.mock_all_auths().set_proxy_wallet(&proxy_wallet);

    assert_eq!(token1.balance(&destination), 0);
    assert_eq!(token2.balance(&destination), 0);
    assert_eq!(token3.balance(&destination), 0);

    // approve tokens for proxy wallet & then lock it
    token1
        .mock_all_auths()
        .approve(&proxy_wallet, &swap_pool.address, &i128::MAX, &9999);

    // init swap
    let operation_id = 1;
    let token_in = tokens[0].clone();
    let swaps_chain = Vec::from_array(
        &e,
        [
            (tokens1.clone(), pool_index1.clone(), tokens[1].clone()),
            (tokens2.clone(), pool_index2.clone(), tokens[2].clone()),
        ],
    );
    token1_admin.mock_all_auths().mint(&proxy_wallet, &100);

    assert_eq!(swap_pool.get_requests(&destination), Vec::new(&e));
    assert_eq!(
        swap_pool.get_completed_requests(&destination, &0),
        Vec::new(&e)
    );
    assert_eq!(swap_pool.get_destinations(&0), Vec::new(&e));

    let memo = swap_pool.generate_user_memo(&destination, &tokens[2]);

    swap_pool
        .mock_auths(&[MockAuth {
            address: &operator,
            invoke: &MockAuthInvoke {
                contract: &swap_pool.address,
                fn_name: "add_request",
                args: Vec::from_array(
                    &e,
                    [
                        operator.to_val(),
                        BytesN::from_array(&e, &[0; 32]).into_val(&e),
                        operation_id.into_val(&e),
                        memo.to_val(),
                        token_in.to_val(),
                        100_i128.into_val(&e),
                    ],
                )
                .into_val(&e),
                sub_invokes: &[],
            },
        }])
        .add_request(
            &operator,
            &BytesN::from_array(&e, &[0; 32]),
            &operation_id,
            &memo,
            &token_in,
            &100,
        );

    // check storage
    assert_eq!(
        swap_pool.get_requests(&destination),
        Vec::from_array(
            &e,
            [(
                BytesN::from_array(&e, &[0; 32]),
                operation_id,
                destination.clone(),
                token_in.clone(),
                100,
                tokens[2].clone(),
            ),]
        )
    );
    assert_eq!(swap_pool.get_completed_requests_last_page(&destination), 0);
    assert_eq!(
        swap_pool.get_completed_requests(&destination, &0),
        Vec::new(&e)
    );
    assert_eq!(swap_pool.get_destinations_last_page(), 0);
    assert_eq!(
        swap_pool.get_destinations(&0),
        vec![&e, destination.clone()]
    );

    let amount_out = swap_pool
        .mock_auths(&[MockAuth {
            address: &operator,
            invoke: &MockAuthInvoke {
                contract: &swap_pool.address,
                fn_name: "swap_chained_via_router",
                args: Vec::from_array(
                    &e,
                    [
                        operator.to_val(),
                        destination.to_val(),
                        operation_id.into_val(&e),
                        swaps_chain.to_val(),
                        95_i128.into_val(&e),
                    ],
                )
                .into_val(&e),
                sub_invokes: &[],
            },
        }])
        .swap_chained_via_router(&operator, &destination, &operation_id, &swaps_chain, &95);
    assert_eq!(amount_out, 96);
    assert_eq!(
        e.auths(),
        std::vec![(
            operator.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    swap_pool.address.clone(),
                    Symbol::new(&e, "swap_chained_via_router"),
                    Vec::from_array(
                        &e,
                        [
                            operator.to_val(),
                            destination.to_val(),
                            operation_id.into_val(&e),
                            swaps_chain.to_val(),
                            95_i128.into_val(&e),
                        ]
                    )
                )),
                sub_invocations: std::vec![],
            }
        ),]
    );
    assert_eq!(token1.balance(&destination), 0);
    assert_eq!(token2.balance(&destination), 0);
    assert_eq!(token3.balance(&destination), 96);

    // check storage
    assert_eq!(swap_pool.get_requests(&destination), Vec::new(&e));
    assert_eq!(swap_pool.get_completed_requests_last_page(&destination), 0);
    assert_eq!(
        swap_pool.get_completed_requests(&destination, &0),
        Vec::from_array(
            &e,
            [(
                BytesN::from_array(&e, &[0; 32]),
                operation_id,
                destination.clone(),
                token_in.clone(),
                100,
                tokens[2].clone(),
                96,
            ),]
        )
    );
    assert_eq!(swap_pool.get_destinations_last_page(), 0);
    assert_eq!(
        swap_pool.get_destinations(&0),
        vec![&e, destination.clone()]
    );
}

#[test]
fn test_duplicate_destination() {
    let e = Env::default();
    e.budget().reset_unlimited();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let proxy_wallet = Address::generate(&e);
    let operator = Address::generate(&e);
    let destination = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let _token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token1_admin = SorobanTokenAdminClient::new(&e, &tokens[0]);
    let token2_admin = SorobanTokenAdminClient::new(&e, &tokens[1]);

    let tokens1 = Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]);

    // init swap router with all it's complexity
    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = deploy_plane_contract(&e);
    let swap_router = deploy_swap_calculator_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane);
    let router = deploy_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&token1.address);
    router.set_pools_plane(&admin, &plane);
    router.set_swap_router(&admin, &swap_router.address);

    // init pools & deposit
    let (pool_index1, _pool_address1) = router.init_standard_pool(&admin, &tokens1, &30);
    token1_admin.mint(&admin, &10000);
    token2_admin.mint(&admin, &20000);
    router.deposit(
        &admin,
        &tokens1,
        &pool_index1,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );

    // init current contract
    let swap_pool = deploy_swap_pool(&e);
    swap_pool.set_admin(&admin);
    swap_pool.set_operator(&operator);
    swap_pool.set_swap_router(&router.address);
    swap_pool.set_proxy_wallet(&proxy_wallet);

    // approve tokens for proxy wallet & then lock it
    token1.approve(&proxy_wallet, &swap_pool.address, &i128::MAX, &9999);

    // init swap
    let mut operation_id = 1;
    let token_in = tokens[0].clone();
    let token_out = tokens[1].clone();
    let swaps_chain = Vec::from_array(
        &e,
        [(tokens1.clone(), pool_index1.clone(), tokens[1].clone())],
    );
    token1_admin.mint(&proxy_wallet, &200);

    assert_eq!(swap_pool.get_destinations(&0), Vec::new(&e));

    let memo = swap_pool.generate_user_memo(&destination, &token_out);
    swap_pool.add_request(
        &operator,
        &BytesN::from_array(&e, &[0; 32]),
        &operation_id,
        &memo,
        &token_in,
        &100,
    );
    swap_pool.swap_chained_via_router(&operator, &destination, &operation_id, &swaps_chain, &90);

    operation_id += 1;
    swap_pool.add_request(
        &operator,
        &BytesN::from_array(&e, &[0; 32]),
        &operation_id,
        &memo,
        &token_in,
        &100,
    );
    swap_pool.swap_chained_via_router(&operator, &destination, &operation_id, &swaps_chain, &90);

    // check storage
    assert_eq!(swap_pool.get_destinations_last_page(), 0);
    assert_eq!(
        swap_pool.get_destinations(&0),
        vec![&e, destination.clone()]
    );
}
