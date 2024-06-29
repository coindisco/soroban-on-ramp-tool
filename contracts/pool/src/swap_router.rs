use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation};
use soroban_sdk::{vec, Address, BytesN, Env, IntoVal, Symbol, Vec};

pub mod swap_router {
    soroban_sdk::contractimport!(file = "../../wasm/soroban_liquidity_pool_router_contract.wasm");
}

pub(crate) fn swap_with_router(
    e: &Env,
    router: &Address,
    swaps_chain: &Vec<(Vec<Address>, BytesN<32>, Address)>,
    token_in: &Address,
    in_amount: &u128,
    out_min: &u128,
) -> u128 {
    e.authorize_as_current_contract(vec![
        &e,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token_in.clone(),
                fn_name: Symbol::new(e, "transfer"),
                args: Vec::from_array(
                    e,
                    [
                        e.current_contract_address().to_val(),
                        router.clone().to_val(),
                        (*in_amount as i128).into_val(e),
                    ],
                )
                .into_val(e),
            },
            sub_invocations: Vec::new(e),
        }),
    ]);

    swap_router::Client::new(e, router).swap_chained(
        &e.current_contract_address(),
        swaps_chain,
        token_in,
        in_amount,
        out_min,
    )
}
