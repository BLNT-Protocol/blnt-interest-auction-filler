use soroban_sdk::{
    contract, contractimpl, contracttype, map,
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Vec,
};

use crate::{
    AuctionData, BackstopTier, BlntInterestAuctionFiller, BlntInterestAuctionFillerClient, Request,
};

#[derive(Clone)]
#[contracttype]
enum CometKey {
    Token0,
    Token1,
    Balance(Address),
    Allowance(Address, Address),
}

#[contract]
struct MockComet;

#[contractimpl]
impl MockComet {
    pub fn __constructor(e: Env, token0: Address, token1: Address) {
        e.storage().instance().set(&CometKey::Token0, &token0);
        e.storage().instance().set(&CometKey::Token1, &token1);
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&CometKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&CometKey::Allowance(from, spender))
            .unwrap_or(0)
    }

    pub fn approve(e: Env, from: Address, spender: Address, amount: i128, _expiration: u32) {
        from.require_auth();
        assert!(amount >= 0);
        e.storage()
            .persistent()
            .set(&CometKey::Allowance(from, spender), &amount);
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        move_shares(&e, &from, &to, amount);
    }

    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        let key = CometKey::Allowance(from.clone(), spender);
        let allowance: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        assert!(amount >= 0 && allowance >= amount);
        e.storage().persistent().set(&key, &(allowance - amount));
        move_shares(&e, &from, &to, amount);
    }

    pub fn dep_lp_tokn_amt_out_get_tokn_in(
        e: Env,
        token_in: Address,
        pool_amount_out: i128,
        max_amount_in: i128,
        user: Address,
    ) -> i128 {
        user.require_auth();
        require_bound(&e, &token_in);
        let input = pool_amount_out.checked_mul(2).unwrap();
        assert!(input > 0 && input <= max_amount_in);
        pull_underlying(&e, &token_in, &user, input, max_amount_in);
        add_shares(&e, &user, pool_amount_out);
        input
    }

    #[allow(clippy::too_many_arguments)]
    pub fn swap_exact_amount_out(
        e: Env,
        token_in: Address,
        max_amount_in: i128,
        token_out: Address,
        token_amount_out: i128,
        _max_price: i128,
        user: Address,
    ) -> (i128, i128) {
        user.require_auth();
        require_bound(&e, &token_in);
        require_bound(&e, &token_out);
        assert!(token_in != token_out);
        let input = token_amount_out.checked_mul(2).unwrap();
        assert!(input > 0 && input <= max_amount_in);
        pull_underlying(&e, &token_in, &user, input, max_amount_in);
        TokenClient::new(&e, &token_out).transfer(
            &e.current_contract_address(),
            &user,
            &token_amount_out,
        );
        (input, 1)
    }
}

fn require_bound(e: &Env, token: &Address) {
    let token0: Address = e.storage().instance().get(&CometKey::Token0).unwrap();
    let token1: Address = e.storage().instance().get(&CometKey::Token1).unwrap();
    assert!(*token == token0 || *token == token1);
}

fn pull_underlying(e: &Env, token: &Address, user: &Address, amount: i128, max_amount: i128) {
    let approval_ledger = (e.ledger().sequence() / 100_000 + 1) * 100_000;
    let client = TokenClient::new(e, token);
    client.approve(
        user,
        &e.current_contract_address(),
        &max_amount,
        &approval_ledger,
    );
    client.transfer_from(
        &e.current_contract_address(),
        user,
        &e.current_contract_address(),
        &amount,
    );
}

fn add_shares(e: &Env, to: &Address, amount: i128) {
    let key = CometKey::Balance(to.clone());
    let balance: i128 = e.storage().persistent().get(&key).unwrap_or(0);
    e.storage().persistent().set(&key, &(balance + amount));
}

fn move_shares(e: &Env, from: &Address, to: &Address, amount: i128) {
    assert!(amount >= 0);
    let from_key = CometKey::Balance(from.clone());
    let from_balance: i128 = e.storage().persistent().get(&from_key).unwrap_or(0);
    assert!(from_balance >= amount);
    e.storage()
        .persistent()
        .set(&from_key, &(from_balance - amount));
    add_shares(e, to, amount);
}

#[derive(Clone)]
#[contracttype]
enum BackstopKey {
    BlntUsdc,
    BlntXlm,
    Usdc,
    Xlm,
}

#[contract]
struct MockBackstop;

#[contractimpl]
impl MockBackstop {
    pub fn __constructor(
        e: Env,
        blnt_usdc: Address,
        blnt_xlm: Address,
        usdc: Address,
        xlm: Address,
    ) {
        e.storage()
            .instance()
            .set(&BackstopKey::BlntUsdc, &blnt_usdc);
        e.storage().instance().set(&BackstopKey::BlntXlm, &blnt_xlm);
        e.storage().instance().set(&BackstopKey::Usdc, &usdc);
        e.storage().instance().set(&BackstopKey::Xlm, &xlm);
    }

    pub fn donate(e: Env, tier: BackstopTier, from: Address, pool: Address, amount: i128) {
        from.require_auth();
        pool.require_auth();
        let key = match tier {
            BackstopTier::FirstLoss => BackstopKey::BlntUsdc,
            BackstopTier::SecondLoss => BackstopKey::BlntXlm,
            BackstopTier::ThirdLoss => BackstopKey::Usdc,
        };
        let token: Address = e.storage().instance().get(&key).unwrap();
        TokenClient::new(&e, &token).transfer_from(
            &e.current_contract_address(),
            &from,
            &e.current_contract_address(),
            &amount,
        );
    }
}

#[derive(Clone)]
#[contracttype]
enum PoolKey {
    AuctionType,
    Backstop,
    Bid,
    BidAmount,
    Block,
    Fail,
    Lot,
    LotAmount,
    Tier,
}

#[contract]
struct MockPool;

#[derive(Clone)]
#[contracttype]
struct MockPoolConfig {
    auction_type: u32,
    backstop: Address,
    bid: Address,
    bid_amount: i128,
    block: u32,
    fail: bool,
    lot: Address,
    lot_amount: i128,
    tier: BackstopTier,
}

#[contractimpl]
impl MockPool {
    pub fn __constructor(e: Env, config: MockPoolConfig) {
        e.storage()
            .instance()
            .set(&PoolKey::AuctionType, &config.auction_type);
        e.storage()
            .instance()
            .set(&PoolKey::Backstop, &config.backstop);
        e.storage().instance().set(&PoolKey::Bid, &config.bid);
        e.storage()
            .instance()
            .set(&PoolKey::BidAmount, &config.bid_amount);
        e.storage().instance().set(&PoolKey::Block, &config.block);
        e.storage().instance().set(&PoolKey::Lot, &config.lot);
        e.storage()
            .instance()
            .set(&PoolKey::LotAmount, &config.lot_amount);
        e.storage().instance().set(&PoolKey::Tier, &config.tier);
        e.storage().instance().set(&PoolKey::Fail, &config.fail);
    }

    pub fn get_auction(e: Env, auction_type: u32, user: Address) -> AuctionData {
        assert_eq!(
            auction_type,
            e.storage()
                .instance()
                .get::<PoolKey, u32>(&PoolKey::AuctionType)
                .unwrap()
        );
        assert_eq!(
            user,
            e.storage().instance().get(&PoolKey::Backstop).unwrap()
        );
        AuctionData {
            bid: map![
                &e,
                (
                    e.storage().instance().get(&PoolKey::Bid).unwrap(),
                    e.storage().instance().get(&PoolKey::BidAmount).unwrap(),
                )
            ],
            lot: map![
                &e,
                (
                    e.storage().instance().get(&PoolKey::Lot).unwrap(),
                    e.storage().instance().get(&PoolKey::LotAmount).unwrap(),
                )
            ],
            block: e.storage().instance().get(&PoolKey::Block).unwrap(),
        }
    }

    pub fn submit(e: Env, from: Address, _spender: Address, to: Address, requests: Vec<Request>) {
        from.require_auth();
        assert_eq!(requests.len(), 1);
        let request = requests.get(0).unwrap();
        let backstop: Address = e.storage().instance().get(&PoolKey::Backstop).unwrap();
        assert_eq!(request.address, backstop);
        let bid: Address = e.storage().instance().get(&PoolKey::Bid).unwrap();
        let bid_amount: i128 = e.storage().instance().get(&PoolKey::BidAmount).unwrap();
        match request.request_type {
            8 => MockBackstopClient::new(&e, &backstop).donate(
                &e.storage().instance().get(&PoolKey::Tier).unwrap(),
                &from,
                &e.current_contract_address(),
                &bid_amount,
            ),
            10 => TokenClient::new(&e, &bid).transfer(
                &from,
                e.current_contract_address(),
                &bid_amount,
            ),
            _ => panic!("unexpected request type"),
        }
        let lot: Address = e.storage().instance().get(&PoolKey::Lot).unwrap();
        let lot_amount: i128 = e.storage().instance().get(&PoolKey::LotAmount).unwrap();
        TokenClient::new(&e, &lot).transfer(&e.current_contract_address(), &to, &lot_amount);
        assert!(!e.storage().instance().get(&PoolKey::Fail).unwrap_or(false));
    }
}

#[contract]
struct MockFactory;

#[contractimpl]
impl MockFactory {
    pub fn __constructor(e: Env, pool: Address) {
        e.storage().instance().set(&0_u32, &pool);
    }

    pub fn is_pool(e: Env, pool: Address) -> bool {
        e.storage().instance().get::<u32, Address>(&0_u32) == Some(pool)
    }
}

#[derive(Clone, Copy)]
enum Route {
    Usdc,
    BlntUsdc,
    Xlm,
    BlntXlm,
    UnsupportedBlntInterest,
    ProtocolFee,
}

struct Fixture {
    e: Env,
    owner: Address,
    user: Address,
    filler: Address,
    pool: Address,
    backstop: Address,
    blnt: Address,
    usdc: Address,
    lot: Address,
    bid: Address,
}

fn token(e: &Env) -> (Address, Address) {
    let admin = Address::generate(e);
    let registered = e.register_stellar_asset_contract_v2(admin.clone());
    (registered.address(), admin)
}

fn fixture(route: Route, fail: bool) -> Fixture {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();
    e.ledger().with_mut(|ledger| ledger.sequence_number = 100);
    let owner = Address::generate(&e);
    let user = Address::generate(&e);
    let (blnt, blnt_admin) = token(&e);
    let (usdc, usdc_admin) = token(&e);
    let (xlm, xlm_admin) = token(&e);
    let (lot, lot_admin) = token(&e);
    let blnt_usdc = e.register(MockComet, (&blnt, &usdc));
    let blnt_xlm = e.register(MockComet, (&blnt, &xlm));
    let (auction_type, bid, tier) = match route {
        Route::Usdc => (2, usdc.clone(), BackstopTier::ThirdLoss),
        Route::BlntUsdc => (2, blnt_usdc.clone(), BackstopTier::FirstLoss),
        Route::Xlm => (2, xlm.clone(), BackstopTier::ThirdLoss),
        Route::BlntXlm => (2, blnt_xlm.clone(), BackstopTier::SecondLoss),
        Route::UnsupportedBlntInterest => (2, blnt.clone(), BackstopTier::FirstLoss),
        Route::ProtocolFee => (3, blnt.clone(), BackstopTier::FirstLoss),
    };
    let (first, second, third) = match tier {
        BackstopTier::FirstLoss => (bid.clone(), blnt_xlm.clone(), usdc.clone()),
        BackstopTier::SecondLoss => (blnt_usdc.clone(), bid.clone(), usdc.clone()),
        BackstopTier::ThirdLoss => (blnt_usdc.clone(), blnt_xlm.clone(), bid.clone()),
    };
    let backstop = e.register(MockBackstop, (&first, &second, &third, &xlm));
    let pool = e.register(
        MockPool,
        (MockPoolConfig {
            auction_type,
            backstop: backstop.clone(),
            bid: bid.clone(),
            bid_amount: 100,
            block: 100,
            fail,
            lot: lot.clone(),
            lot_amount: 50,
            tier,
        },),
    );
    let factory = e.register(MockFactory, (&pool,));
    let filler = e.register(
        BlntInterestAuctionFiller,
        (
            &owner, &factory, &backstop, &blnt, &usdc, &xlm, &blnt_usdc, &blnt_xlm,
        ),
    );

    StellarAssetClient::new(&e, &usdc).mint(&owner, &10_000);
    StellarAssetClient::new(&e, &usdc).mint(&user, &10_000);
    StellarAssetClient::new(&e, &blnt).mint(&blnt_usdc, &10_000);
    StellarAssetClient::new(&e, &xlm).mint(&blnt_xlm, &10_000);
    StellarAssetClient::new(&e, &lot).mint(&pool, &1_000);
    let _ = (blnt_admin, usdc_admin, xlm_admin, lot_admin);

    Fixture {
        e,
        owner,
        user,
        filler,
        pool,
        backstop,
        blnt,
        usdc,
        lot,
        bid,
    }
}

fn fill_interest(f: &Fixture, max_usdc: i128, max_blnt: i128) {
    BlntInterestAuctionFillerClient::new(&f.e, &f.filler)
        .fill_interest(&f.owner, &f.pool, &100, &max_usdc, &max_blnt, &100, &f.bid);
}

#[test]
fn fills_all_four_interest_tier_asset_routes() {
    for (route, max_usdc, max_blnt, spent_usdc, refunded_blnt) in [
        (Route::Usdc, 100, 0, 100, 0),
        (Route::BlntUsdc, 250, 0, 200, 0),
        (Route::Xlm, 500, 250, 500, 50),
        (Route::BlntXlm, 500, 250, 500, 50),
    ] {
        let f = fixture(route, false);
        fill_interest(&f, max_usdc, max_blnt);
        assert_eq!(
            TokenClient::new(&f.e, &f.usdc).balance(&f.owner),
            10_000 - spent_usdc
        );
        assert_eq!(
            TokenClient::new(&f.e, &f.blnt).balance(&f.owner),
            refunded_blnt
        );
        assert_eq!(TokenClient::new(&f.e, &f.bid).balance(&f.backstop), 100);
        assert_eq!(TokenClient::new(&f.e, &f.lot).balance(&f.filler), 50);
        assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.filler), 0);
        assert_eq!(TokenClient::new(&f.e, &f.blnt).balance(&f.filler), 0);
    }
}

#[test]
fn fills_protocol_fee_auction_from_usdc() {
    let f = fixture(Route::ProtocolFee, false);
    BlntInterestAuctionFillerClient::new(&f.e, &f.filler)
        .fill_protocol_fee(&f.owner, &f.pool, &100, &250, &100);
    assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.owner), 9_800);
    assert_eq!(TokenClient::new(&f.e, &f.blnt).balance(&f.pool), 100);
    assert_eq!(TokenClient::new(&f.e, &f.lot).balance(&f.filler), 50);
}

#[test]
fn failed_fill_rolls_back_swaps_mint_and_input() {
    let f = fixture(Route::BlntXlm, true);
    let result = BlntInterestAuctionFillerClient::new(&f.e, &f.filler)
        .try_fill_interest(&f.user, &f.pool, &100, &500, &250, &100, &f.bid);
    assert!(result.is_err());
    assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.owner), 10_000);
    assert_eq!(TokenClient::new(&f.e, &f.blnt).balance(&f.owner), 0);
    assert_eq!(TokenClient::new(&f.e, &f.bid).balance(&f.backstop), 0);
    assert_eq!(TokenClient::new(&f.e, &f.lot).balance(&f.filler), 0);
}

#[test]
fn auction_identity_and_input_caps_are_enforced() {
    let f = fixture(Route::BlntUsdc, false);
    let client = BlntInterestAuctionFillerClient::new(&f.e, &f.filler);
    assert!(client
        .try_fill_interest(&f.user, &f.pool, &100, &250, &0, &99, &f.bid)
        .is_err());
    assert!(client
        .try_fill_interest(&f.user, &f.pool, &100, &199, &0, &100, &f.bid)
        .is_err());
    assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.owner), 10_000);
}

#[test]
fn rejects_non_owner_funding_address() {
    let f = fixture(Route::Usdc, false);
    let result = BlntInterestAuctionFillerClient::new(&f.e, &f.filler)
        .try_fill_interest(&f.user, &f.pool, &100, &100, &0, &100, &f.bid);
    assert!(result.is_err());
    assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.user), 10_000);
    assert_eq!(TokenClient::new(&f.e, &f.lot).balance(&f.filler), 0);
}

#[test]
fn rejects_noncanonical_interest_token_and_unregistered_pool() {
    let f = fixture(Route::UnsupportedBlntInterest, false);
    let client = BlntInterestAuctionFillerClient::new(&f.e, &f.filler);
    assert!(client
        .try_fill_interest(&f.owner, &f.pool, &100, &250, &0, &100, &f.bid)
        .is_err());
    assert!(client
        .try_fill_interest(
            &f.owner,
            &Address::generate(&f.e),
            &100,
            &250,
            &0,
            &100,
            &f.bid,
        )
        .is_err());
    assert_eq!(TokenClient::new(&f.e, &f.usdc).balance(&f.owner), 10_000);
}

#[test]
fn owner_can_claim_successful_lot() {
    let f = fixture(Route::Usdc, false);
    fill_interest(&f, 100, 0);
    let amounts = BlntInterestAuctionFillerClient::new(&f.e, &f.filler)
        .claim(&f.owner, &vec![&f.e, f.lot.clone()]);
    assert_eq!(amounts, vec![&f.e, 50_i128]);
    assert_eq!(TokenClient::new(&f.e, &f.lot).balance(&f.owner), 50);
}
