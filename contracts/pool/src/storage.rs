use crate::constants::{COMPLETED_REQUESTS_PAGE_SIZE, DEFAULT_MEMO, DESTINATIONS_PAGE_SIZE};
use crate::memo;
use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, String, Vec};
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
    ProxyWallet,
    Operator,
    SwapRouter,
    SwapRequests(Address),
    LastOperationId,
    CompletedSwapRequests(Address, u32),
    CompletedSwapRequestLastPage(Address),
    DestinationsList(u32),
    DestinationsLastPage,
    UserMemo(Address, Address),
    UserTokenByMemo(String),
    NextMemo,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedSwapRequest {
    pub tx_id: BytesN<32>,
    pub op_id: u128,
    pub destination: Address,
    pub token_in: Address,
    pub amount_in: i128,
    pub token_out: Address,
    pub amount_out: i128,
}

generate_instance_storage_getter_and_setter!(operator, DataKey::Operator, Address);
generate_instance_storage_getter_and_setter!(swap_router, DataKey::SwapRouter, Address);
generate_instance_storage_getter_and_setter!(proxy_wallet, DataKey::ProxyWallet, Address);
generate_instance_storage_getter_and_setter_with_default!(
    last_operation_id,
    DataKey::LastOperationId,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    destinations_last_page,
    DataKey::DestinationsLastPage,
    u32,
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

pub fn is_new_destination(e: &Env, destination: &Address) -> bool {
    let key = DataKey::SwapRequests(destination.clone());
    !e.storage().persistent().has(&key)
}

pub fn set_active_swap_requests(e: &Env, destination: &Address, value: &Vec<SwapRequest>) {
    let key = DataKey::SwapRequests(destination.clone());
    e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
}

pub fn add_swap_request(e: &Env, destination: &Address, value: &SwapRequest) {
    if is_new_destination(e, destination) {
        add_destination(e, destination);
    }

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

pub fn get_completed_swap_requests_last_page(e: &Env, destination: &Address) -> u32 {
    let key = DataKey::CompletedSwapRequestLastPage(destination.clone());
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => 0,
    }
}

pub fn set_completed_swap_requests_last_page(e: &Env, destination: &Address, value: u32) {
    let key = DataKey::CompletedSwapRequestLastPage(destination.clone());
    e.storage().persistent().set(&key, &value);
    bump_persistent(e, &key);
}

pub fn get_completed_swap_requests_page(
    e: &Env,
    destination: &Address,
    page: u32,
) -> Vec<CompletedSwapRequest> {
    let key = DataKey::CompletedSwapRequests(destination.clone(), page);
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => Vec::new(e),
    }
}

pub fn set_completed_swap_requests_page(
    e: &Env,
    destination: &Address,
    page: u32,
    value: &Vec<CompletedSwapRequest>,
) {
    let key = DataKey::CompletedSwapRequests(destination.clone(), page);
    e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
}

pub fn add_completed_swap_request(e: &Env, destination: &Address, value: CompletedSwapRequest) {
    let last_page = get_completed_swap_requests_last_page(e, destination);
    let mut requests = get_completed_swap_requests_page(e, destination, last_page);
    requests.push_back(value);
    set_completed_swap_requests_page(e, destination, last_page, &requests);
    if requests.len() == COMPLETED_REQUESTS_PAGE_SIZE {
        set_completed_swap_requests_last_page(e, destination, last_page + 1);
    }
}

pub fn set_swap_request_processed(
    e: &Env,
    destination: &Address,
    swap_request: SwapRequest,
    amount_out: i128,
) {
    let mut requests = get_active_swap_requests(e, destination);
    let index = requests.last_index_of(swap_request.clone());
    match index {
        Some(index) => {
            requests.remove(index);
            set_active_swap_requests(e, destination, &requests);
            add_completed_swap_request(
                e,
                destination,
                CompletedSwapRequest {
                    tx_id: swap_request.tx_id,
                    op_id: swap_request.op_id,
                    destination: swap_request.destination,
                    token_in: swap_request.token_in,
                    amount_in: swap_request.amount_in,
                    token_out: swap_request.token_out,
                    amount_out,
                },
            );
        }
        None => panic_with_error!(e, StorageError::ValueMissing),
    }
}

pub fn get_destinations(e: &Env, page: u32) -> Vec<Address> {
    let key = DataKey::DestinationsList(page);
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => Vec::new(e),
    }
}

pub fn set_destinations(e: &Env, page: u32, value: &Vec<Address>) {
    let key = DataKey::DestinationsList(page);
    e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
}

pub fn add_destination(e: &Env, destination: &Address) {
    let last_page = get_destinations_last_page(e);
    let mut destinations = get_destinations(e, last_page);
    destinations.push_back(destination.clone());
    set_destinations(e, last_page, &destinations);
    if destinations.len() == DESTINATIONS_PAGE_SIZE {
        set_destinations_last_page(e, &(last_page + 1));
    }
}

fn set_next_memo(e: &Env, value: &String) {
    bump_instance(e);
    let key = DataKey::NextMemo;
    e.storage().instance().set(&key, value);
}

fn get_next_memo(e: &Env) -> String {
    let key = DataKey::NextMemo;
    match e.storage().instance().get(&key) {
        Some(v) => v,
        None => String::from_str(e, DEFAULT_MEMO),
    }
}

pub fn set_user_token_by_memo(e: &Env, memo: &String, user: &Address, token: &Address) {
    let key = DataKey::UserTokenByMemo(memo.clone());
    e.storage().persistent().set(&key, &(user, token));
    bump_persistent(e, &key);
}

pub fn get_user_token_by_memo(e: &Env, memo: &String) -> (Address, Address) {
    let key = DataKey::UserTokenByMemo(memo.clone());
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => {
            panic_with_error!(e, StorageError::ValueMissing)
        }
    }
}

pub fn set_user_memo(e: &Env, user: &Address, token: &Address, value: &String) {
    let key = DataKey::UserMemo(user.clone(), token.clone());
    e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
}

fn generate_next_memo(e: &Env, memo_str: &String) -> String {
    let mut memo_bytes = [0u8; 28];
    memo_str.copy_into_slice(&mut memo_bytes);
    String::from_bytes(e, &memo::generate_next_memo(&memo_bytes))
}

pub fn get_user_memo(e: &Env, user: &Address, token: &Address) -> String {
    let key = DataKey::UserMemo(user.clone(), token.clone());
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => {
            let memo = get_next_memo(e);
            set_user_memo(e, user, token, &memo);
            set_user_token_by_memo(e, &memo, user, token);
            set_next_memo(e, &generate_next_memo(e, &memo));
            memo
        }
    }
}
