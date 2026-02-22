mod strat_common;

use evore::state::{StrategyDeployer, strategy_deployer_pda};
use evore::validation::{StrategyType, validate_strategy_data};
use solana_sdk::pubkey::Pubkey;

// ============================================================================
// StrategyDeployer struct layout
// ============================================================================

#[test]
fn test_strat_deployer_struct_size_is_176() {
    assert_eq!(
        std::mem::size_of::<StrategyDeployer>(),
        176,
        "StrategyDeployer struct must be exactly 176 bytes (184 with 8-byte discriminator)"
    );
}

// ============================================================================
// PDA derivation
// ============================================================================

#[test]
fn test_strategy_deployer_pda_is_deterministic() {
    let manager_key = Pubkey::new_unique();
    let (pda1, bump1) = strategy_deployer_pda(manager_key);
    let (pda2, bump2) = strategy_deployer_pda(manager_key);
    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
}

#[test]
fn test_strategy_deployer_pda_differs_per_manager() {
    let manager_a = Pubkey::new_unique();
    let manager_b = Pubkey::new_unique();
    let (pda_a, _) = strategy_deployer_pda(manager_a);
    let (pda_b, _) = strategy_deployer_pda(manager_b);
    assert_ne!(pda_a, pda_b, "Different managers must produce different PDAs");
}

#[test]
fn test_strategy_deployer_pda_uses_correct_seeds() {
    let manager_key = Pubkey::new_unique();
    let (pda, bump) = strategy_deployer_pda(manager_key);

    let expected = Pubkey::create_program_address(
        &[b"strategy-deployer", &manager_key.to_bytes(), &[bump]],
        &evore::id(),
    )
    .unwrap();

    assert_eq!(pda, expected, "PDA must use seeds [\"strategy-deployer\", manager_key]");
}

// ============================================================================
// StrategyType discriminants
// ============================================================================

#[test]
fn test_strategy_type_from_u8() {
    assert_eq!(StrategyType::try_from(0).unwrap(), StrategyType::Ev);
    assert_eq!(StrategyType::try_from(1).unwrap(), StrategyType::Percentage);
    assert_eq!(StrategyType::try_from(2).unwrap(), StrategyType::Manual);
    assert_eq!(StrategyType::try_from(3).unwrap(), StrategyType::Split);
    assert_eq!(StrategyType::try_from(4).unwrap(), StrategyType::DynamicSplitPercentage);
    assert_eq!(StrategyType::try_from(5).unwrap(), StrategyType::DynamicEv);
}

#[test]
fn test_invalid_strategy_type_fails() {
    assert!(StrategyType::try_from(6).is_err());
    assert!(StrategyType::try_from(255).is_err());
}

// ============================================================================
// EV strategy validation
// ============================================================================

fn ev_data(max_per_square: u64, min_bet: u64, slots_left: u64, ore_value: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&max_per_square.to_le_bytes());
    d[8..16].copy_from_slice(&min_bet.to_le_bytes());
    d[16..24].copy_from_slice(&slots_left.to_le_bytes());
    d[24..32].copy_from_slice(&ore_value.to_le_bytes());
    d
}

#[test]
fn test_ev_valid_data() {
    let data = ev_data(100_000, 1_000, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Ev, &data).is_ok());
}

#[test]
fn test_ev_zero_max_per_square_fails() {
    let data = ev_data(0, 1_000, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Ev, &data).is_err());
}

#[test]
fn test_ev_zero_min_bet_fails() {
    let data = ev_data(100_000, 0, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Ev, &data).is_err());
}

#[test]
fn test_ev_zero_slots_left_ok() {
    let data = ev_data(100_000, 1_000, 0, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Ev, &data).is_ok());
}

#[test]
fn test_ev_zero_ore_value_ok() {
    let data = ev_data(100_000, 1_000, 50, 0);
    assert!(validate_strategy_data(StrategyType::Ev, &data).is_ok());
}

// ============================================================================
// Percentage strategy validation
// ============================================================================

fn pct_data(percentage: u64, squares_count: u64, ml_min: u64, ml_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&percentage.to_le_bytes());
    d[8..16].copy_from_slice(&squares_count.to_le_bytes());
    d[16..24].copy_from_slice(&ml_min.to_le_bytes());
    d[24..32].copy_from_slice(&ml_max.to_le_bytes());
    d
}

#[test]
fn test_percentage_valid_data() {
    let data = pct_data(1000, 5, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_ok());
}

#[test]
fn test_percentage_zero_pct_fails() {
    let data = pct_data(0, 5, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_err());
}

#[test]
fn test_percentage_over_10000_fails() {
    let data = pct_data(10_001, 5, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_err());
}

#[test]
fn test_percentage_zero_squares_fails() {
    let data = pct_data(1000, 0, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_err());
}

#[test]
fn test_percentage_over_25_squares_fails() {
    let data = pct_data(1000, 26, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_err());
}

#[test]
fn test_percentage_motherlode_min_gt_max_fails() {
    let data = pct_data(1000, 5, 2_000_000_000, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_err());
}

#[test]
fn test_percentage_motherlode_zeros_ok() {
    let data = pct_data(1000, 5, 0, 0);
    assert!(validate_strategy_data(StrategyType::Percentage, &data).is_ok());
}

// ============================================================================
// Manual strategy validation
// ============================================================================

#[test]
fn test_manual_any_data_valid() {
    let data = [0xFFu8; 64];
    assert!(validate_strategy_data(StrategyType::Manual, &data).is_ok());
}

// ============================================================================
// Split strategy validation
// ============================================================================

fn split_data(ml_min: u64, ml_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&ml_min.to_le_bytes());
    d[8..16].copy_from_slice(&ml_max.to_le_bytes());
    d
}

#[test]
fn test_split_valid_data() {
    let data = split_data(500_000_000, 2_000_000_000);
    assert!(validate_strategy_data(StrategyType::Split, &data).is_ok());
}

#[test]
fn test_split_motherlode_min_gt_max_fails() {
    let data = split_data(2_000_000_000, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::Split, &data).is_err());
}

#[test]
fn test_split_motherlode_zeros_ok() {
    let data = split_data(0, 0);
    assert!(validate_strategy_data(StrategyType::Split, &data).is_ok());
}

// ============================================================================
// DynamicSplitPercentage strategy validation
// ============================================================================

fn dsp_data(pct: u64, mask: u64, ml_min: u64, ml_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&pct.to_le_bytes());
    d[8..16].copy_from_slice(&mask.to_le_bytes());
    d[16..24].copy_from_slice(&ml_min.to_le_bytes());
    d[24..32].copy_from_slice(&ml_max.to_le_bytes());
    d
}

#[test]
fn test_dsp_valid_data() {
    let data = dsp_data(1000, 0x1FF_FFFF, 0, 0);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_ok());
}

#[test]
fn test_dsp_zero_pct_fails() {
    let data = dsp_data(0, 0x1FF_FFFF, 0, 0);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_err());
}

#[test]
fn test_dsp_over_10000_pct_fails() {
    let data = dsp_data(10_001, 0x1FF_FFFF, 0, 0);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_err());
}

#[test]
fn test_dsp_zero_mask_fails() {
    let data = dsp_data(1000, 0, 0, 0);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_err());
}

#[test]
fn test_dsp_invalid_bits_fails() {
    let data = dsp_data(1000, 1 << 25, 0, 0);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_err());
}

#[test]
fn test_dsp_motherlode_min_gt_max_fails() {
    let data = dsp_data(1000, 0x1FF_FFFF, 2_000_000_000, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::DynamicSplitPercentage, &data).is_err());
}

// ============================================================================
// DynamicEv strategy validation
// ============================================================================

fn dynev_data(max_per_square: u64, min_bet: u64, slots_left: u64, max_ore_value: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&max_per_square.to_le_bytes());
    d[8..16].copy_from_slice(&min_bet.to_le_bytes());
    d[16..24].copy_from_slice(&slots_left.to_le_bytes());
    d[24..32].copy_from_slice(&max_ore_value.to_le_bytes());
    d
}

#[test]
fn test_dynev_valid_data() {
    let data = dynev_data(100_000, 1_000, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::DynamicEv, &data).is_ok());
}

#[test]
fn test_dynev_zero_max_per_square_fails() {
    let data = dynev_data(0, 1_000, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::DynamicEv, &data).is_err());
}

#[test]
fn test_dynev_zero_min_bet_fails() {
    let data = dynev_data(100_000, 0, 50, 1_000_000_000);
    assert!(validate_strategy_data(StrategyType::DynamicEv, &data).is_err());
}

#[test]
fn test_dynev_zero_max_ore_value_ok() {
    let data = dynev_data(100_000, 1_000, 50, 0);
    assert!(validate_strategy_data(StrategyType::DynamicEv, &data).is_ok());
}
