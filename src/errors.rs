use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FillerError {
    InvalidConfig = 1,
    InvalidAmount = 2,
    InvalidAuction = 3,
    UnsupportedBidToken = 4,
    PoolNotRegistered = 5,
    BalanceError = 6,
    OverflowError = 7,
    Unauthorized = 8,
}
