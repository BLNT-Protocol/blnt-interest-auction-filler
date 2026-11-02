use soroban_sdk::{contracttype, Address, Map};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct AuctionData {
    pub bid: Map<Address, i128>,
    pub lot: Map<Address, i128>,
    pub block: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum BackstopTier {
    FirstLoss,
    SecondLoss,
    ThirdLoss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Request {
    pub address: Address,
    pub amount: i128,
    pub request_type: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Config {
    pub backstop: Address,
    pub blnt: Address,
    pub blnt_usdc: Address,
    pub blnt_xlm: Address,
    pub pool_factory: Address,
    pub usdc: Address,
    pub xlm: Address,
}
