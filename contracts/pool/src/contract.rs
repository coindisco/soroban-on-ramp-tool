use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Vec};

use access_control::access::{AccessControl, AccessControlTrait};

use crate::errors::PoolError;
use crate::interfaces::{PoolContractInterface, UpgradeableContract};
use crate::swap_router::swap_with_router;

use crate::storage::{
    add_swap_request, get_active_swap_requests, get_completed_swap_requests_last_page,
    get_completed_swap_requests_page, get_destinations, get_destinations_last_page,
    get_last_operation_id, get_operational_fee, get_operator, get_swap_request_by_id,
    get_swap_router, get_token_in, set_operational_fee, set_operator, set_swap_request_processed,
    set_swap_router, set_token_in, SwapRequest,
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

    fn set_token_in(e: Env, admin: Address, token_in: Address) {
        admin.require_auth();
        AccessControl::new(&e).check_admin(&admin);

        set_token_in(&e, &token_in);
    }

    fn set_operator(e: Env, admin: Address, operator: Address) {
        admin.require_auth();
        AccessControl::new(&e).check_admin(&admin);

        set_operator(&e, &operator);
    }

    fn set_swap_router(e: Env, admin: Address, swap_router: Address) {
        admin.require_auth();
        AccessControl::new(&e).check_admin(&admin);

        set_swap_router(&e, &swap_router);
    }

    fn add_request(
        e: Env,
        operator: Address,
        proxy_wallet: Address,
        token_out: Address,
        tx_id: BytesN<32>,
        op_id: u128,
        destination: Address,
        amount_in: i128,
    ) {
        // check operator is whitelisted
        operator.require_auth();
        if operator != get_operator(&e) {
            panic_with_error!(&e, PoolError::UnauthorizedOperator);
        }

        // check if operation id already consumed
        if op_id <= get_last_operation_id(&e) {
            panic_with_error!(&e, PoolError::OperationIdAlreadyConsumed);
        }

        let token_in_client = SorobanTokenClient::new(&e, &get_token_in(&e));
        token_in_client.transfer_from(
            &e.current_contract_address(),
            &proxy_wallet,
            &e.current_contract_address(),
            &amount_in,
        );

        let operational_fee = get_operational_fee(&e);
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
            &get_token_in(&e),
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
    fn get_token_in(e: Env) -> Address {
        get_token_in(&e)
    }

    fn get_last_operation_id(e: Env) -> u128 {
        get_last_operation_id(&e)
    }

    fn get_requests(
        e: Env,
        destination: Address,
    ) -> Vec<(BytesN<32>, u128, Address, i128, Address)> {
        let mut result = Vec::new(&e);
        for request in get_active_swap_requests(&e, &destination) {
            result.push_back((
                request.tx_id,
                request.op_id,
                request.destination,
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
    ) -> Vec<(BytesN<32>, u128, Address, i128, Address, i128)> {
        let mut result = Vec::new(&e);
        for request in get_completed_swap_requests_page(&e, &destination, page) {
            result.push_back((
                request.tx_id,
                request.op_id,
                request.destination,
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

    fn get_operational_fee(e: Env) -> i128 {
        get_operational_fee(&e)
    }

    fn set_operational_fee(e: Env, operator: Address, fee: i128) {
        operator.require_auth();
        if operator != get_operator(&e) {
            panic_with_error!(&e, PoolError::UnauthorizedOperator);
        }

        set_operational_fee(&e, &fee);
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
