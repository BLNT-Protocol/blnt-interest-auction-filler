use soroban_sdk::{contracttype, panic_with_error, Address, Env};

use crate::{Config, FillerError};

const ONE_DAY_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 25 * ONE_DAY_LEDGERS;
const INSTANCE_TTL_BUMP: u32 = 30 * ONE_DAY_LEDGERS;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Config,
    Owner,
}

pub fn initialize(e: &Env, owner: &Address, config: &Config) {
    if e.storage().instance().has(&DataKey::Config) {
        panic_with_error!(e, FillerError::InvalidConfig);
    }
    e.storage().instance().set(&DataKey::Owner, owner);
    e.storage().instance().set(&DataKey::Config, config);
    extend_instance(e);
}

pub fn owner(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&DataKey::Owner)
        .unwrap_or_else(|| panic_with_error!(e, FillerError::InvalidConfig))
}

pub fn set_owner(e: &Env, owner: &Address) {
    e.storage().instance().set(&DataKey::Owner, owner);
    extend_instance(e);
}

pub fn config(e: &Env) -> Config {
    e.storage()
        .instance()
        .get(&DataKey::Config)
        .unwrap_or_else(|| panic_with_error!(e, FillerError::InvalidConfig))
}

pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_BUMP);
}
