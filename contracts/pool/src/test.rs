#![cfg(test)]
use super::*;
use crate::swap_router::swap_router;
use soroban_sdk::testutils::arbitrary::std;
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, MockAuth, MockAuthInvoke,
};
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, Map, Symbol, Vec};

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
    swap_pool
        .mock_all_auths()
        .add_proxy_wallet(&proxy_wallet, &tokens[2]);

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
                        proxy_wallet.to_val(),
                        BytesN::from_array(&e, &[0; 32]).into_val(&e),
                        operation_id.into_val(&e),
                        destination.to_val(),
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
            &proxy_wallet,
            &BytesN::from_array(&e, &[0; 32]),
            &operation_id,
            &destination,
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
    swap_pool.add_proxy_wallet(&proxy_wallet, &tokens[1]);

    // approve tokens for proxy wallet & then lock it
    token1.approve(&proxy_wallet, &swap_pool.address, &i128::MAX, &9999);

    // init swap
    let mut operation_id = 1;
    let token_in = tokens[0].clone();
    let swaps_chain = Vec::from_array(
        &e,
        [(tokens1.clone(), pool_index1.clone(), tokens[1].clone())],
    );
    token1_admin.mint(&proxy_wallet, &200);

    assert_eq!(swap_pool.get_destinations(&0), Vec::new(&e));

    swap_pool.add_request(
        &operator,
        &proxy_wallet,
        &BytesN::from_array(&e, &[0; 32]),
        &operation_id,
        &destination,
        &token_in,
        &100,
    );
    swap_pool.swap_chained_via_router(&operator, &destination, &operation_id, &swaps_chain, &90);

    operation_id += 1;
    swap_pool.add_request(
        &operator,
        &proxy_wallet,
        &BytesN::from_array(&e, &[0; 32]),
        &operation_id,
        &destination,
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

#[test]
fn test_overwrite_wallet() {
    let e = Env::default();
    e.budget().reset_unlimited();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let proxy_wallet1 = Address::generate(&e);
    let proxy_wallet2 = Address::generate(&e);
    let proxy_wallet3 = Address::generate(&e);
    let token1 = create_token_contract(&e, &admin).address;
    let token2 = create_token_contract(&e, &admin).address;

    let swap_pool = deploy_swap_pool(&e);
    swap_pool.set_admin(&admin);
    swap_pool.add_proxy_wallet(&proxy_wallet1, &token1);
    assert_eq!(
        swap_pool.get_proxy_wallets(),
        Map::from_array(&e, [(proxy_wallet1, token1.clone())])
    );
    swap_pool.add_proxy_wallet(&proxy_wallet2, &token1);
    assert_eq!(
        swap_pool.get_proxy_wallets(),
        Map::from_array(&e, [(proxy_wallet2.clone(), token1.clone())])
    );
    swap_pool.add_proxy_wallet(&proxy_wallet3, &token2);
    assert_eq!(
        swap_pool.get_proxy_wallets(),
        Map::from_array(
            &e,
            [
                (proxy_wallet2, token1.clone()),
                (proxy_wallet3, token2.clone())
            ]
        )
    );
}

#[should_panic(expected = "Error(Contract, #2303)")]
#[test]
fn test_unregistered_proxy_wallet() {
    let e = Env::default();
    e.budget().reset_unlimited();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let proxy_wallet = Address::generate(&e);
    let operator = Address::generate(&e);
    let destination = Address::generate(&e);
    let token_in = create_token_contract(&e, &admin).address;
    let token_out = create_token_contract(&e, &admin).address;

    // init current contract
    let swap_pool = deploy_swap_pool(&e);
    swap_pool.set_admin(&admin);
    swap_pool.set_operator(&operator);
    swap_pool.add_proxy_wallet(&Address::generate(&e), &token_out);

    swap_pool.add_request(
        &operator,
        &proxy_wallet,
        &BytesN::from_array(&e, &[0; 32]),
        &1,
        &destination,
        &token_in,
        &100,
    );
}
