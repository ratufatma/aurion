use aurion_primitives::quantum::Quantum;

pub const QUANTA_PER_AUR: u128 = 100_000_000;
pub const MAX_TOTAL_SUPPLY_AUR: u128 = 66_000_000;
pub const MAX_TOTAL_SUPPLY_QUANTA: Quantum =
    Quantum::new(MAX_TOTAL_SUPPLY_AUR.saturating_mul(QUANTA_PER_AUR));

pub const CREATOR_ALLOCATION_AUR: u128 = 19_800_000;
pub const DEV_ALLOCATION_AUR: u128 = 6_600_000;
pub const TOTAL_GENESIS_PREMINE_QUANTA: Quantum =
    Quantum::new((CREATOR_ALLOCATION_AUR.saturating_add(DEV_ALLOCATION_AUR)).saturating_mul(QUANTA_PER_AUR));

pub const INITIAL_SUBSIDY_AUR: u128 = 99;
pub const INITIAL_SUBSIDY_QUANTA: Quantum =
    Quantum::new(INITIAL_SUBSIDY_AUR.saturating_mul(QUANTA_PER_AUR));
pub const SUBSIDY_HALVING_INTERVAL: u64 = 200_000;
pub const MAX_HALVINGS: u64 = 64;

// Backward-compatible aliases for legacy references
pub const INITIAL_SUBSIDY: u128 = INITIAL_SUBSIDY_QUANTA.raw();
pub const HALVING_INTERVAL: u64 = SUBSIDY_HALVING_INTERVAL;

pub fn calculate_block_subsidy(height: u64) -> Quantum {
    let halvings = height
        .checked_div(SUBSIDY_HALVING_INTERVAL)
        .unwrap_or_default();

    if halvings >= MAX_HALVINGS {
        return Quantum::ZERO;
    }

    let raw = INITIAL_SUBSIDY_QUANTA.raw() >> halvings;
    Quantum::from_raw(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsidy_halving_schedule() {
        assert_eq!(calculate_block_subsidy(0), INITIAL_SUBSIDY_QUANTA);
        assert_eq!(
            calculate_block_subsidy(SUBSIDY_HALVING_INTERVAL.saturating_sub(1)),
            INITIAL_SUBSIDY_QUANTA
        );
        assert_eq!(
            calculate_block_subsidy(SUBSIDY_HALVING_INTERVAL),
            Quantum::from_raw(INITIAL_SUBSIDY_QUANTA.raw() >> 1)
        );
        assert_eq!(
            calculate_block_subsidy(SUBSIDY_HALVING_INTERVAL.saturating_mul(2)),
            Quantum::from_raw(INITIAL_SUBSIDY_QUANTA.raw() >> 2)
        );
        assert_eq!(
            calculate_block_subsidy(MAX_HALVINGS.saturating_mul(SUBSIDY_HALVING_INTERVAL)),
            Quantum::ZERO
        );
    }

    #[test]
    fn test_cumulative_emission_does_not_exceed_hard_cap() {
        let mut cumulative_pow_quanta = 0u128;
        for era in 0..MAX_HALVINGS {
            let subsidy = INITIAL_SUBSIDY_QUANTA.raw() >> era;
            let era_emission = subsidy.saturating_mul(SUBSIDY_HALVING_INTERVAL as u128);
            cumulative_pow_quanta = cumulative_pow_quanta.saturating_add(era_emission);
        }

        let total_supply_emitted =
            cumulative_pow_quanta.saturating_add(TOTAL_GENESIS_PREMINE_QUANTA.raw());
        assert!(total_supply_emitted <= MAX_TOTAL_SUPPLY_QUANTA.raw());
        assert_eq!(total_supply_emitted, 6_599_999_998_000_000);
    }
}
