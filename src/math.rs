use soroban_sdk::{panic_with_error, Env, I256};

use crate::FillerError;

const SCALAR_7: i128 = 10_000_000;
const PER_LEDGER: i128 = 50_000;

pub fn scaled_bid_amount(e: &Env, amount: i128, percent: u32, block: u32) -> i128 {
    if amount <= 0 || percent == 0 || percent > 100 {
        panic_with_error!(e, FillerError::InvalidAuction);
    }
    let elapsed = e
        .ledger()
        .sequence()
        .checked_sub(block)
        .unwrap_or_else(|| panic_with_error!(e, FillerError::InvalidAuction));
    let modifier = if elapsed <= 200 {
        SCALAR_7
    } else if elapsed < 400 {
        SCALAR_7
            .checked_sub(
                i128::from(elapsed - 200)
                    .checked_mul(PER_LEDGER)
                    .unwrap_or_else(|| panic_with_error!(e, FillerError::OverflowError)),
            )
            .unwrap_or_else(|| panic_with_error!(e, FillerError::OverflowError))
    } else {
        0
    };
    let percent_scaled = i128::from(percent)
        .checked_mul(100_000)
        .unwrap_or_else(|| panic_with_error!(e, FillerError::OverflowError));
    let selected = mul_ceil(e, amount, percent_scaled, SCALAR_7);
    mul_ceil(e, selected, modifier, SCALAR_7)
}

fn mul_ceil(e: &Env, value: i128, numerator: i128, denominator: i128) -> i128 {
    if value < 0 || numerator < 0 || denominator <= 0 {
        panic_with_error!(e, FillerError::OverflowError);
    }
    let denominator = I256::from_i128(e, denominator);
    I256::from_i128(e, value)
        .mul(&I256::from_i128(e, numerator))
        .add(&denominator)
        .sub(&I256::from_i32(e, 1))
        .div(&denominator)
        .to_i128()
        .unwrap_or_else(|| panic_with_error!(e, FillerError::OverflowError))
}

#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::Ledger, Env};

    use super::scaled_bid_amount;

    fn env_at(sequence: u32) -> Env {
        let e = Env::default();
        e.ledger()
            .with_mut(|ledger| ledger.sequence_number = sequence);
        e
    }

    #[test]
    fn v3_curve_boundaries_and_rounding() {
        assert_eq!(
            scaled_bid_amount(&env_at(100), 10_000_001, 100, 100),
            10_000_001
        );
        assert_eq!(
            scaled_bid_amount(&env_at(300), 10_000_001, 100, 100),
            10_000_001
        );
        assert_eq!(
            scaled_bid_amount(&env_at(301), 10_000_001, 100, 100),
            9_950_001
        );
        assert_eq!(
            scaled_bid_amount(&env_at(400), 10_000_001, 100, 100),
            5_000_001
        );
        assert_eq!(scaled_bid_amount(&env_at(500), 10_000_001, 100, 100), 0);
        assert_eq!(scaled_bid_amount(&env_at(100), 3, 50, 100), 2);
    }
}
