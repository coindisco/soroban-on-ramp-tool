use crate::errors::PoolError;
use crate::storage::{
    add_swap_request, get_last_operation_id, get_operator, get_swap_request_by_id, get_swap_router,
    set_operator, set_swap_request_processed, set_swap_router, SwapRequest,
};
use crate::swap_router::swap_with_router;
use access_control::access::{AccessControl, AccessControlTrait};
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Vec};

#[contract]
pub struct PoolContract;

pub trait PoolContractInterface {
    fn set_admin(e: Env, admin: Address);

    fn set_operator(e: Env, operator: Address);

    fn set_swap_router(e: Env, swap_router: Address);

    fn request_swap(
        e: Env,
        proxy_wallet: Address,
        tx_id: BytesN<32>,
        op_id: u128,
        destination: Address,
        token_in: Address,
        amount_in: i128,
        token_out: Address,
    );

    fn swap_chained_via_router(
        e: Env,
        operator: Address,
        destination: Address,
        op_id: u128,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        out_min: i128,
    ) -> i128; // getters
               // get_swap by id
               // get operator
               // get swap router

    fn get_last_operation_id(e: Env) -> u128;
}

#[contractimpl]
impl PoolContractInterface for PoolContract {
    // admin methods
    // init: set admin
    // set swap router
    // set operator (the account capable of adding new swaps & executing them)
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

    fn set_swap_router(e: Env, swap_router: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_swap_router(&e, &swap_router);
    }

    fn request_swap(
        e: Env,
        proxy_wallet: Address,
        tx_id: BytesN<32>,
        op_id: u128,
        destination: Address,
        token_in: Address,
        amount_in: i128,
        token_out: Address,
    ) {
        proxy_wallet.require_auth();
        // todo: check if proxy_wallet is allowed

        // check if operation id already consumed
        if op_id <= get_last_operation_id(&e) {
            panic_with_error!(&e, PoolError::OperationIdAlreadyConsumed);
        }

        SorobanTokenClient::new(&e, &token_in).transfer(
            &proxy_wallet,
            &e.current_contract_address(),
            &amount_in,
        );

        add_swap_request(
            &e,
            &destination,
            &SwapRequest {
                tx_id,
                op_id,
                destination: destination.clone(),
                token_in,
                amount_in,
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

    // getters
    // get_swap by id
    // get operator
    // get swap router

    fn get_last_operation_id(e: Env) -> u128 {
        get_last_operation_id(&e)
    }
}
