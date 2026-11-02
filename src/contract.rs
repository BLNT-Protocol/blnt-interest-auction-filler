use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractimpl, panic_with_error, symbol_short,
    token::TokenClient,
    vec, Address, Env, IntoVal, Val, Vec,
};

use crate::{
    dependencies::{CometClient, PoolClient, PoolFactoryClient},
    math::scaled_bid_amount,
    storage, AuctionData, BackstopTier, Config, FillerError, Request,
};

const INTEREST_AUCTION_TYPE: u32 = 2;
const PROTOCOL_FEE_AUCTION_TYPE: u32 = 3;
const FILL_INTEREST_REQUEST_TYPE: u32 = 8;
const FILL_PROTOCOL_FEE_REQUEST_TYPE: u32 = 10;

#[contract]
pub struct BlntInterestAuctionFiller;

#[contractimpl]
impl BlntInterestAuctionFiller {
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        e: Env,
        owner: Address,
        pool_factory: Address,
        backstop: Address,
        blnt: Address,
        usdc: Address,
        xlm: Address,
        blnt_usdc: Address,
        blnt_xlm: Address,
    ) {
        let addresses = vec![
            &e,
            pool_factory.clone(),
            backstop.clone(),
            blnt.clone(),
            usdc.clone(),
            xlm.clone(),
            blnt_usdc.clone(),
            blnt_xlm.clone(),
        ];
        for i in 0..addresses.len() {
            for j in (i + 1)..addresses.len() {
                if addresses.get(i).unwrap() == addresses.get(j).unwrap() {
                    panic_with_error!(&e, FillerError::InvalidConfig);
                }
            }
        }
        storage::initialize(
            &e,
            &owner,
            &Config {
                backstop,
                blnt,
                blnt_usdc,
                blnt_xlm,
                pool_factory,
                usdc,
                xlm,
            },
        );
    }

    pub fn get_config(e: Env) -> Config {
        storage::extend_instance(&e);
        storage::config(&e)
    }

    pub fn get_owner(e: Env) -> Address {
        storage::extend_instance(&e);
        storage::owner(&e)
    }

    pub fn set_owner(e: Env, new_owner: Address) {
        let owner = storage::owner(&e);
        owner.require_auth();
        storage::set_owner(&e, &new_owner);
    }

    /// Atomically fund and fill any canonical BLNT V3 interest-auction tier.
    ///
    /// `max_blnt_in` is required only for XLM and BLNT:XLM bids. It is the
    /// exact BLNT intermediate acquired from BLNT:USDC and the maximum BLNT
    /// that the following BLNT:XLM operation may consume. Any unused USDC or
    /// BLNT is returned before the pool fill is invoked.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_interest(
        e: Env,
        from: Address,
        pool: Address,
        fill_percent: u32,
        max_usdc_in: i128,
        max_blnt_in: i128,
        expected_auction_block: u32,
        expected_bid_token: Address,
    ) {
        execute_fill(
            &e,
            &from,
            &pool,
            INTEREST_AUCTION_TYPE,
            FILL_INTEREST_REQUEST_TYPE,
            fill_percent,
            max_usdc_in,
            max_blnt_in,
            expected_auction_block,
            &expected_bid_token,
        );
    }

    /// Atomically buy the exact BLNT bid from USDC and fill a protocol-fee
    /// auction. The pool burns the BLNT and sends the realized reserve lot to
    /// this helper for owner claim.
    pub fn fill_protocol_fee(
        e: Env,
        from: Address,
        pool: Address,
        fill_percent: u32,
        max_usdc_in: i128,
        expected_auction_block: u32,
    ) {
        let config = storage::config(&e);
        execute_fill(
            &e,
            &from,
            &pool,
            PROTOCOL_FEE_AUCTION_TYPE,
            FILL_PROTOCOL_FEE_REQUEST_TYPE,
            fill_percent,
            max_usdc_in,
            0,
            expected_auction_block,
            &config.blnt,
        );
    }

    pub fn claim(e: Env, to: Address, assets: Vec<Address>) -> Vec<i128> {
        storage::extend_instance(&e);
        storage::owner(&e).require_auth();
        let current = e.current_contract_address();
        let mut amounts = Vec::new(&e);
        for asset in assets {
            let token = TokenClient::new(&e, &asset);
            let amount = token.balance(&current);
            if amount > 0 {
                token.transfer(&current, &to, &amount);
            }
            amounts.push_back(amount);
        }
        amounts
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_fill(
    e: &Env,
    from: &Address,
    pool: &Address,
    auction_type: u32,
    request_type: u32,
    fill_percent: u32,
    max_usdc_in: i128,
    max_blnt_in: i128,
    expected_block: u32,
    expected_token: &Address,
) {
    storage::extend_instance(e);
    from.require_auth();
    if fill_percent == 0
        || fill_percent > 100
        || max_usdc_in < 0
        || max_blnt_in < 0
        || !matches!(
            auction_type,
            INTEREST_AUCTION_TYPE | PROTOCOL_FEE_AUCTION_TYPE
        )
    {
        panic_with_error!(e, FillerError::InvalidAmount);
    }

    let config = storage::config(e);
    if *from != storage::owner(e) {
        panic_with_error!(e, FillerError::Unauthorized);
    }
    if !PoolFactoryClient::new(e, &config.pool_factory).is_pool(pool) {
        panic_with_error!(e, FillerError::PoolNotRegistered);
    }

    let auction = PoolClient::new(e, pool).get_auction(&auction_type, &config.backstop);
    let (bid_token, bid_amount) =
        validate_and_scale_auction(e, &auction, fill_percent, expected_block, expected_token);
    if auction_type == INTEREST_AUCTION_TYPE
        && bid_token != config.usdc
        && bid_token != config.xlm
        && bid_token != config.blnt_usdc
        && bid_token != config.blnt_xlm
    {
        panic_with_error!(e, FillerError::UnsupportedBidToken);
    }
    if auction_type == PROTOCOL_FEE_AUCTION_TYPE && bid_token != config.blnt {
        panic_with_error!(e, FillerError::UnsupportedBidToken);
    }

    if bid_amount > 0 {
        acquire_bid_token(
            e,
            &config,
            from,
            &bid_token,
            bid_amount,
            max_usdc_in,
            max_blnt_in,
        );
        if auction_type == INTEREST_AUCTION_TYPE {
            TokenClient::new(e, &bid_token).approve(
                &e.current_contract_address(),
                &config.backstop,
                &bid_amount,
                &e.ledger().sequence(),
            );
            authorize_interest_donation(e, &config.backstop, pool, bid_amount);
        } else {
            authorize_transfer(e, &config.blnt, pool, bid_amount);
        }
    }

    let current = e.current_contract_address();
    let requests = vec![
        e,
        Request {
            address: config.backstop,
            amount: i128::from(fill_percent),
            request_type,
        },
    ];
    let args: Vec<Val> = vec![
        e,
        current.clone().into_val(e),
        current.clone().into_val(e),
        current.into_val(e),
        requests.into_val(e),
    ];
    let _: Val = e.invoke_contract(pool, &symbol_short!("submit"), args);
}

fn validate_and_scale_auction(
    e: &Env,
    auction: &AuctionData,
    fill_percent: u32,
    expected_block: u32,
    expected_token: &Address,
) -> (Address, i128) {
    if auction.block != expected_block || auction.bid.len() != 1 {
        panic_with_error!(e, FillerError::InvalidAuction);
    }
    let (token, amount) = auction
        .bid
        .iter()
        .next()
        .unwrap_or_else(|| panic_with_error!(e, FillerError::InvalidAuction));
    if token != *expected_token || amount <= 0 {
        panic_with_error!(e, FillerError::InvalidAuction);
    }
    (
        token,
        scaled_bid_amount(e, amount, fill_percent, auction.block),
    )
}

fn acquire_bid_token(
    e: &Env,
    config: &Config,
    from: &Address,
    bid_token: &Address,
    bid_amount: i128,
    max_usdc_in: i128,
    max_blnt_in: i128,
) {
    if max_usdc_in <= 0 {
        panic_with_error!(e, FillerError::InvalidAmount);
    }
    let current = e.current_contract_address();
    let usdc = TokenClient::new(e, &config.usdc);
    if *bid_token == config.usdc {
        if bid_amount > max_usdc_in || max_blnt_in != 0 {
            panic_with_error!(e, FillerError::InvalidAmount);
        }
        let usdc_before = usdc.balance(&current);
        usdc.transfer(from, &current, &max_usdc_in);
        refund_delta(e, &config.usdc, from, usdc_before, bid_amount);
        return;
    }

    let usdc_before = usdc.balance(&current);
    let blnt = TokenClient::new(e, &config.blnt);
    let blnt_before = blnt.balance(&current);
    usdc.transfer(from, &current, &max_usdc_in);

    if *bid_token == config.blnt_usdc {
        if max_blnt_in != 0 {
            panic_with_error!(e, FillerError::InvalidAmount);
        }
        deposit_exact_lp(e, &config.blnt_usdc, &config.usdc, bid_amount, max_usdc_in);
    } else if *bid_token == config.blnt {
        if max_blnt_in != 0 {
            panic_with_error!(e, FillerError::InvalidAmount);
        }
        swap_exact_out(
            e,
            &config.blnt_usdc,
            &config.usdc,
            max_usdc_in,
            &config.blnt,
            bid_amount,
        );
    } else if *bid_token == config.xlm || *bid_token == config.blnt_xlm {
        if max_blnt_in <= 0 {
            panic_with_error!(e, FillerError::InvalidAmount);
        }
        swap_exact_out(
            e,
            &config.blnt_usdc,
            &config.usdc,
            max_usdc_in,
            &config.blnt,
            max_blnt_in,
        );
        if *bid_token == config.xlm {
            swap_exact_out(
                e,
                &config.blnt_xlm,
                &config.blnt,
                max_blnt_in,
                &config.xlm,
                bid_amount,
            );
        } else {
            deposit_exact_lp(e, &config.blnt_xlm, &config.blnt, bid_amount, max_blnt_in);
        }
    } else {
        panic_with_error!(e, FillerError::UnsupportedBidToken);
    }

    refund_delta(e, &config.usdc, from, usdc_before, 0);
    let retained_blnt = if *bid_token == config.blnt {
        bid_amount
    } else {
        0
    };
    refund_delta(e, &config.blnt, from, blnt_before, retained_blnt);
}

fn deposit_exact_lp(e: &Env, pool: &Address, token: &Address, output: i128, max_input: i128) {
    authorize_comet_approval(e, token, pool, max_input);
    let input = CometClient::new(e, pool).dep_lp_tokn_amt_out_get_tokn_in(
        token,
        &output,
        &max_input,
        &e.current_contract_address(),
    );
    if input <= 0 || input > max_input {
        panic_with_error!(e, FillerError::BalanceError);
    }
}

fn swap_exact_out(
    e: &Env,
    pool: &Address,
    token_in: &Address,
    max_input: i128,
    token_out: &Address,
    output: i128,
) {
    authorize_comet_approval(e, token_in, pool, max_input);
    let (input, _) = CometClient::new(e, pool).swap_exact_amount_out(
        token_in,
        &max_input,
        token_out,
        &output,
        &i128::MAX,
        &e.current_contract_address(),
    );
    if input <= 0 || input > max_input {
        panic_with_error!(e, FillerError::BalanceError);
    }
}

fn refund_delta(e: &Env, token: &Address, to: &Address, before: i128, retained: i128) {
    let client = TokenClient::new(e, token);
    let refund = client
        .balance(&e.current_contract_address())
        .checked_sub(before)
        .and_then(|value| value.checked_sub(retained))
        .unwrap_or_else(|| panic_with_error!(e, FillerError::BalanceError));
    if refund > 0 {
        client.transfer(&e.current_contract_address(), to, &refund);
    }
}

fn authorize_comet_approval(e: &Env, token: &Address, pool: &Address, amount: i128) {
    let approval_ledger = e
        .ledger()
        .sequence()
        .checked_div(100_000)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_mul(100_000))
        .unwrap_or_else(|| panic_with_error!(e, FillerError::OverflowError));
    e.authorize_as_current_contract(vec![
        e,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token.clone(),
                fn_name: symbol_short!("approve"),
                args: vec![
                    e,
                    e.current_contract_address().into_val(e),
                    pool.clone().into_val(e),
                    amount.into_val(e),
                    approval_ledger.into_val(e),
                ],
            },
            sub_invocations: vec![e],
        }),
    ]);
}

fn authorize_interest_donation(e: &Env, backstop: &Address, pool: &Address, amount: i128) {
    let current = e.current_contract_address();
    let mut entries = Vec::new(e);
    for tier in [
        BackstopTier::FirstLoss,
        BackstopTier::SecondLoss,
        BackstopTier::ThirdLoss,
    ] {
        entries.push_back(InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: backstop.clone(),
                fn_name: symbol_short!("donate"),
                args: vec![
                    e,
                    tier.into_val(e),
                    current.clone().into_val(e),
                    pool.clone().into_val(e),
                    amount.into_val(e),
                ],
            },
            sub_invocations: vec![e],
        }));
    }
    e.authorize_as_current_contract(entries);
}

fn authorize_transfer(e: &Env, token: &Address, to: &Address, amount: i128) {
    e.authorize_as_current_contract(vec![
        e,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: token.clone(),
                fn_name: symbol_short!("transfer"),
                args: vec![
                    e,
                    e.current_contract_address().into_val(e),
                    to.clone().into_val(e),
                    amount.into_val(e),
                ],
            },
            sub_invocations: vec![e],
        }),
    ]);
}
