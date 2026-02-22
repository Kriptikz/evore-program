use solana_program::program_error::ProgramError;

use crate::error::EvoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StrategyType {
    Ev = 0,
    Percentage = 1,
    Manual = 2,
    Split = 3,
    DynamicSplitPercentage = 4,
    DynamicEv = 5,
}

impl TryFrom<u8> for StrategyType {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StrategyType::Ev),
            1 => Ok(StrategyType::Percentage),
            2 => Ok(StrategyType::Manual),
            3 => Ok(StrategyType::Split),
            4 => Ok(StrategyType::DynamicSplitPercentage),
            5 => Ok(StrategyType::DynamicEv),
            _ => Err(EvoreError::InvalidStrategyType.into()),
        }
    }
}

pub fn validate_strategy_data(strategy_type: StrategyType, strategy_data: &[u8; 64]) -> Result<(), ProgramError> {
    match strategy_type {
        StrategyType::Ev => {
            let max_per_square = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let min_bet = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());

            if max_per_square == 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if min_bet == 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
        }
        StrategyType::Percentage => {
            let percentage = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let squares_count = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());
            let motherlode_min = u64::from_le_bytes(strategy_data[16..24].try_into().unwrap());
            let motherlode_max = u64::from_le_bytes(strategy_data[24..32].try_into().unwrap());

            if percentage == 0 || percentage > 10_000 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if squares_count == 0 || squares_count > 25 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if motherlode_min > 0 && motherlode_max > 0 && motherlode_min > motherlode_max {
                return Err(EvoreError::InvalidStrategyData.into());
            }
        }
        StrategyType::Manual => {}
        StrategyType::Split => {
            let motherlode_min = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let motherlode_max = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());

            if motherlode_min > 0 && motherlode_max > 0 && motherlode_min > motherlode_max {
                return Err(EvoreError::InvalidStrategyData.into());
            }
        }
        StrategyType::DynamicSplitPercentage => {
            let percentage = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let squares_mask = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());
            let motherlode_min = u64::from_le_bytes(strategy_data[16..24].try_into().unwrap());
            let motherlode_max = u64::from_le_bytes(strategy_data[24..32].try_into().unwrap());

            if percentage == 0 || percentage > 10_000 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if squares_mask == 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if squares_mask & !0x1FF_FFFF != 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if motherlode_min > 0 && motherlode_max > 0 && motherlode_min > motherlode_max {
                return Err(EvoreError::InvalidStrategyData.into());
            }
        }
        StrategyType::DynamicEv => {
            let max_per_square = u64::from_le_bytes(strategy_data[0..8].try_into().unwrap());
            let min_bet = u64::from_le_bytes(strategy_data[8..16].try_into().unwrap());

            if max_per_square == 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
            if min_bet == 0 {
                return Err(EvoreError::InvalidStrategyData.into());
            }
        }
    }
    Ok(())
}
