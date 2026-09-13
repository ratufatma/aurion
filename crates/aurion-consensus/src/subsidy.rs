use aurion_primitives::quantum::Quantum;

pub const INITIAL_SUBSIDY_AUR: u128 = 50;
pub const BASE_UNITS_PER_AUR: u128 = 100_000_000;
pub const INITIAL_SUBSIDY: u128 = 5_000_000_000;
pub const HALVING_INTERVAL: u64 = 210_000;

pub fn calculate_block_subsidy(height: u64) -> Quantum {
    let halvings = match height.checked_div(HALVING_INTERVAL) {
        Some(h) => h,
        None => 0,
    };

    if halvings >= 64 {
        return Quantum::ZERO;
    }

    let raw = INITIAL_SUBSIDY >> halvings;
    Quantum::from_raw(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsidy_halving_schedule() {
        assert_eq!(calculate_block_subsidy(0), Quantum::from_raw(5_000_000_000));
        assert_eq!(calculate_block_subsidy(209_999), Quantum::from_raw(5_000_000_000));
        assert_eq!(calculate_block_subsidy(210_000), Quantum::from_raw(2_500_000_000));
        assert_eq!(calculate_block_subsidy(420_000), Quantum::from_raw(1_250_000_000));
        assert_eq!(calculate_block_subsidy(64 * HALVING_INTERVAL), Quantum::ZERO);
    }
}
