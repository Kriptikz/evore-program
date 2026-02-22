use solana_program::program_error::ProgramError;
use steel::*;

use crate::{
    error::EvoreError,
    ore_api::{Board, Round},
    processor::process_mm_deploy::{
        calculate_percentage_deployments, plan_max_profit_waterfill, DeploymentBatch,
    },
    validation::{validate_strategy_data, StrategyType},
};

pub(crate) struct StrategyResult {
    pub batches: Vec<DeploymentBatch>,
    pub total_to_deploy: u64,
    pub needs_automation: bool,
}

/// Dispatch strategy to compute deployment batches.
///
/// Validates strategy data before computing deployments.
/// Returns error on invalid strategy type, invalid data, or if no deployments can be made.
pub(crate) fn dispatch_strategy(
    strategy_type_raw: u8,
    strategy_data: &[u8; 64],
    amount: u64,
    squares_mask: u32,
    extra: u32,
    board: &Board,
    round: &Round,
    clock: &Clock,
) -> Result<StrategyResult, ProgramError> {
    let strategy_type = StrategyType::try_from(strategy_type_raw)?;
    validate_strategy_data(strategy_type, strategy_data)?;

    match strategy_type {
        StrategyType::Ev => {
            let max_per_square = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let min_bet = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());
            let slots_left = u64::from_le_bytes(strategy_data[16..24].try_into().unwrap());
            let ore_value = u64::from_le_bytes(strategy_data[24..32].try_into().unwrap());

            let current_slots_left = board.end_slot - clock.slot;
            if current_slots_left > slots_left {
                return Err(EvoreError::TooManySlotsLeft.into());
            }

            let alloc = plan_max_profit_waterfill(
                round.deployed, amount, min_bet, 100, 10, ore_value, max_per_square,
            );

            let mut ev_batches: Vec<DeploymentBatch> = Vec::new();
            for i in 0..25 {
                if alloc.per_square[i] > 0 {
                    ev_batches.push(DeploymentBatch::single(alloc.per_square[i], i));
                }
            }
            let total = alloc.spent;
            if total == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            Ok(StrategyResult { batches: ev_batches, total_to_deploy: total, needs_automation: true })
        }
        StrategyType::Manual => {
            let mut squares = [false; 25];
            for i in 0..25 {
                if (squares_mask >> i) & 1 == 1 {
                    squares[i] = true;
                }
            }
            let num_squares = squares.iter().filter(|&&s| s).count() as u64;
            if num_squares == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            let total = amount.saturating_mul(num_squares);
            if total == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            Ok(StrategyResult { batches: vec![DeploymentBatch::new(amount, squares)], total_to_deploy: total, needs_automation: false })
        }
        StrategyType::Split => {
            let per_square = amount / 25;
            if per_square == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            let total = per_square * 25;
            Ok(StrategyResult { batches: vec![DeploymentBatch::all_squares(per_square)], total_to_deploy: total, needs_automation: false })
        }
        StrategyType::Percentage => {
            let percentage = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let squares_count = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());
            let bankroll = amount;

            let (batches, total) = calculate_percentage_deployments(round, bankroll, percentage, squares_count);
            if total == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            Ok(StrategyResult { batches, total_to_deploy: total, needs_automation: true })
        }
        StrategyType::DynamicSplitPercentage => {
            let percentage = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let squares_mask_val = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());

            let p = percentage as u128;
            if p == 0 || p >= 10000 {
                return Err(EvoreError::NoDeployments.into());
            }

            let mut dsp_batches = Vec::new();
            let mut total: u64 = 0;
            let bankroll = amount;

            for i in 0..25 {
                if (squares_mask_val >> i) & 1 == 0 { continue; }
                let t = round.deployed[i] as u128;
                if t == 0 { continue; }
                let amount_i = (p * t / (10000 - p)).min(u64::MAX as u128) as u64;
                if amount_i == 0 { continue; }
                if total.saturating_add(amount_i) > bankroll {
                    let remaining = bankroll.saturating_sub(total);
                    if remaining > 0 {
                        dsp_batches.push(DeploymentBatch::single(remaining, i));
                        total = total.saturating_add(remaining);
                    }
                    break;
                }
                dsp_batches.push(DeploymentBatch::single(amount_i, i));
                total = total.saturating_add(amount_i);
            }

            if total == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            Ok(StrategyResult { batches: dsp_batches, total_to_deploy: total, needs_automation: true })
        }
        StrategyType::DynamicEv => {
            let max_ps = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let min_b = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());
            let sl = u64::from_le_bytes(strategy_data[16..24].try_into().unwrap());
            let max_ore = u64::from_le_bytes(strategy_data[24..32].try_into().unwrap());

            let ore_value = (u64::from(extra) << 32) | u64::from(squares_mask);

            if max_ore > 0 && ore_value > max_ore {
                return Err(EvoreError::InvalidStrategyData.into());
            }

            let current_slots_left = board.end_slot.saturating_sub(clock.slot);
            if current_slots_left > sl {
                return Err(EvoreError::TooManySlotsLeft.into());
            }

            let alloc = plan_max_profit_waterfill(
                round.deployed, amount, min_b, 100, 10, ore_value, max_ps,
            );

            let mut dynev_batches: Vec<DeploymentBatch> = Vec::new();
            for i in 0..25 {
                if alloc.per_square[i] > 0 {
                    dynev_batches.push(DeploymentBatch::single(alloc.per_square[i], i));
                }
            }
            let total = alloc.spent;
            if total == 0 {
                return Err(EvoreError::NoDeployments.into());
            }
            Ok(StrategyResult { batches: dynev_batches, total_to_deploy: total, needs_automation: true })
        }
    }
}
