use soroban_sdk::{Address, BytesN, Env, Map, Vec};

pub trait PoolContractInterface {
    fn set_admin(e: Env, admin: Address);

    fn set_operator(e: Env, operator: Address);
    fn add_proxy_wallet(e: Env, proxy_wallet: Address, token_out: Address);
    fn get_proxy_wallets(e: Env) -> Map<Address, Address>;

    fn set_swap_router(e: Env, swap_router: Address);

    fn add_request(
        e: Env,
        operator: Address,
        proxy_wallet: Address,
        tx_id: BytesN<32>,
        op_id: u128,
        destination: Address,
        token_in: Address,
        amount_in: i128,
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
    fn get_requests(
        e: Env,
        destination: Address,
    ) -> Vec<(BytesN<32>, u128, Address, Address, i128, Address)>;
    fn get_completed_requests_last_page(e: Env, destination: Address) -> u32;
    fn get_completed_requests(
        e: Env,
        destination: Address,
        page: u32,
    ) -> Vec<(BytesN<32>, u128, Address, Address, i128, Address, i128)>;
    fn get_destinations_last_page(e: Env) -> u32;
    fn get_destinations(e: Env, page: u32) -> Vec<Address>;

    fn get_operational_fee(e: Env, token: Address) -> i128;
    fn set_operational_fee(e: Env, operator: Address, token: Address, fee: i128);
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}
