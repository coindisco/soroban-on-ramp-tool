use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Map, Vec};

use access_control::access::{AccessControl, AccessControlTrait};

use crate::errors::PoolError;
use crate::interfaces::{PoolContractInterface, UpgradeableContract};
use crate::swap_router::swap_with_router;

use crate::storage::{
    add_proxy_wallet, add_swap_request, get_active_swap_requests,
    get_completed_swap_requests_last_page, get_completed_swap_requests_page, get_destinations,
    get_destinations_last_page, get_last_operation_id, get_operational_fee, get_operator,
    get_proxy_wallets, get_swap_request_by_id, get_swap_router, set_operational_fee, set_operator,
    set_swap_request_processed, set_swap_router, SwapRequest,
};

#[contract]
pub struct PoolContract;

#[contractimpl]
impl PoolContractInterface for PoolContract {
    // admin methods
    fn set_admin(e: Env, admin: Address) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, PoolError::AlreadyInitialized);
        }
        access_control.set_admin(&admin);
    }

    fn set_operator(e: Env, operator: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_operator(&e, &operator);
    }

    fn add_proxy_wallet(e: Env, proxy_wallet: Address, token_out: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        add_proxy_wallet(&e, &proxy_wallet, &token_out);
    }

    fn set_swap_router(e: Env, swap_router: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_swap_router(&e, &swap_router);
    }

    fn add_request(
        e: Env,
        operator: Address,
        proxy_wallet: Address,
        tx_id: BytesN<32>,
        op_id: u128,
        destination: Address,
        token_in: Address,
        amount_in: i128,
    ) {
        // check operator is whitelisted
        operator.require_auth();
        if operator != get_operator(&e) {
            panic_with_error!(&e, PoolError::UnauthorizedOperator);
        }

        let proxy_wallets = get_proxy_wallets(&e);
        let token_out = match proxy_wallets.get(proxy_wallet.clone()) {
            Some(value) => value,
            None => {
                panic_with_error!(&e, PoolError::UnauthorizedProxyWallet);
            }
        };

        // check if operation id already consumed
        if op_id <= get_last_operation_id(&e) {
            panic_with_error!(&e, PoolError::OperationIdAlreadyConsumed);
        }

        let token_in_client = SorobanTokenClient::new(&e, &token_in);
        token_in_client.transfer_from(
            &e.current_contract_address(),
            &proxy_wallet,
            &e.current_contract_address(),
            &amount_in,
        );

        let operational_fee = get_operational_fee(&e, &token_in);
        if operational_fee > 0 {
            token_in_client.transfer(&e.current_contract_address(), &operator, &operational_fee);
        }

        add_swap_request(
            &e,
            &destination,
            &SwapRequest {
                tx_id,
                op_id,
                destination: destination.clone(),
                token_in,
                amount_in: amount_in - operational_fee,
                token_out,
            },
        );

        // todo: emit event
    }

    fn swap_chained_via_router(
        e: Env,
        operator: Address,
        destination: Address,
        op_id: u128,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        out_min: i128,
    ) -> i128 {
        operator.require_auth();
        if operator != get_operator(&e) {
            panic_with_error!(&e, PoolError::UnauthorizedOperator);
        }

        let swap_request = get_swap_request_by_id(&e, &destination, op_id);

        // fulfill request
        let amount_out = swap_with_router(
            &e,
            &get_swap_router(&e),
            &swaps_chain,
            &swap_request.token_in,
            &(swap_request.amount_in as u128),
            &(out_min as u128),
        ) as i128;

        // transfer swap result to destination
        SorobanTokenClient::new(&e, &swap_request.token_out).transfer(
            &e.current_contract_address(),
            &swap_request.destination,
            &amount_out,
        );

        // mark swap as processed
        set_swap_request_processed(&e, &destination, swap_request, amount_out);

        // todo: emit event

        amount_out
    }

    // public getters
    fn get_proxy_wallets(e: Env) -> Map<Address, Address> {
        get_proxy_wallets(&e)
    }

    fn get_last_operation_id(e: Env) -> u128 {
        get_last_operation_id(&e)
    }

    fn get_requests(
        e: Env,
        destination: Address,
    ) -> Vec<(BytesN<32>, u128, Address, Address, i128, Address)> {
        let mut result = Vec::new(&e);
        for request in get_active_swap_requests(&e, &destination) {
            result.push_back((
                request.tx_id,
                request.op_id,
                request.destination,
                request.token_in,
                request.amount_in,
                request.token_out,
            ));
        }
        result
    }

    fn get_completed_requests_last_page(e: Env, destination: Address) -> u32 {
        get_completed_swap_requests_last_page(&e, &destination)
    }

    fn get_completed_requests(
        e: Env,
        destination: Address,
        page: u32,
    ) -> Vec<(BytesN<32>, u128, Address, Address, i128, Address, i128)> {
        let mut result = Vec::new(&e);
        for request in get_completed_swap_requests_page(&e, &destination, page) {
            result.push_back((
                request.tx_id,
                request.op_id,
                request.destination,
                request.token_in,
                request.amount_in,
                request.token_out,
                request.amount_out,
            ));
        }
        result
    }

    fn get_destinations_last_page(e: Env) -> u32 {
        get_destinations_last_page(&e)
    }

    fn get_destinations(e: Env, page: u32) -> Vec<Address> {
        get_destinations(&e, page)
    }

    fn get_operational_fee(e: Env, token: Address) -> i128 {
        get_operational_fee(&e, &token)
    }

    fn set_operational_fee(e: Env, operator: Address, token: Address, fee: i128) {
        operator.require_auth();
        if operator != get_operator(&e) {
            panic_with_error!(&e, PoolError::UnauthorizedOperator);
        }

        set_operational_fee(&e, &token, &fee);
    }
}

#[contractimpl]
impl UpgradeableContract for PoolContract {
    fn version() -> u32 {
        104
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
