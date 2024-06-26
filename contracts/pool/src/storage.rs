use crate::errors::PoolError;
use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    ProxyWallets,
    Operator,
    SwapRouter,
    SwapRequests(Address),
    LastOperationId,
    CompletedSwapRequest(Address, u32),
    CompletedSwapRequestLastPage(Address),
    Destinations(u32),   // todo: keep destinations per page
    DestinationsCounter, // todo: keep destinations total counter
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapRequest {
    pub tx_id: BytesN<32>,
    pub op_id: u128,
    pub destination: Address,
    pub token_in: Address,
    pub amount_in: i128,
    pub token_out: Address,
}

generate_instance_storage_getter_and_setter!(
    proxy_wallets,
    DataKey::ProxyWallets,
    Vec<(Address, Address)>
);
generate_instance_storage_getter_and_setter!(operator, DataKey::Operator, Address);
generate_instance_storage_getter_and_setter!(swap_router, DataKey::SwapRouter, Address);
generate_instance_storage_getter_and_setter_with_default!(
    last_operation_id,
    DataKey::LastOperationId,
    u128,
    0
);

pub fn get_active_swap_requests(e: &Env, destination: &Address) -> Vec<SwapRequest> {
    let key = DataKey::SwapRequests(destination.clone());
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => Vec::new(e),
    }
}

pub fn set_active_swap_requests(e: &Env, destination: &Address, value: &Vec<SwapRequest>) {
    let key = DataKey::SwapRequests(destination.clone());
    let result = e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
    result
}

pub fn add_swap_request(e: &Env, destination: &Address, value: &SwapRequest) {
    let mut requests = get_active_swap_requests(e, destination);
    set_last_operation_id(e, &value.op_id);
    requests.push_back(value.clone());
    set_active_swap_requests(e, destination, &requests);
}

pub fn get_swap_request_by_id(e: &Env, destination: &Address, op_id: u128) -> SwapRequest {
    let requests = get_active_swap_requests(e, destination);
    for request in requests {
        if request.op_id == op_id {
            return request;
        }
    }
    panic_with_error!(e, StorageError::ValueMissing)
}

pub fn set_swap_request_processed(
    e: &Env,
    destination: &Address,
    swap_request: SwapRequest,
    amount_out: i128,
) {
    // don't allow marking swap request as completed if amount out is not set
    if amount_out == 0 {
        panic_with_error!(e, PoolError::SwapNotPerformed);
    }

    let mut requests = get_active_swap_requests(e, destination);
    let index = requests.last_index_of(swap_request.clone());
    match index {
        Some(index) => {
            requests.remove(index);
            set_active_swap_requests(e, destination, &requests);
            // todo: move to completed
            return;
        }
        None => panic_with_error!(e, StorageError::ValueMissing),
    }
}
