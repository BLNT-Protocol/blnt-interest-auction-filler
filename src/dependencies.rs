use soroban_sdk::{contractclient, Address, Env};

use crate::AuctionData;

#[allow(dead_code)]
#[contractclient(name = "PoolClient")]
pub trait Pool {
    fn get_auction(e: Env, auction_type: u32, user: Address) -> AuctionData;
}

#[allow(dead_code)]
#[contractclient(name = "CometClient")]
pub trait Comet {
    fn dep_lp_tokn_amt_out_get_tokn_in(
        e: Env,
        token_in: Address,
        pool_amount_out: i128,
        max_amount_in: i128,
        user: Address,
    ) -> i128;

    fn swap_exact_amount_out(
        e: Env,
        token_in: Address,
        max_amount_in: i128,
        token_out: Address,
        token_amount_out: i128,
        max_price: i128,
        user: Address,
    ) -> (i128, i128);
}
