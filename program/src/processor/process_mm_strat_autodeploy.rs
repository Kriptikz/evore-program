use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};
use steel::*;

use crate::{
    consts::{DEPLOY_FEE, FEE_COLLECTOR, MANAGED_MINER_AUTH, STRATEGY_DEPLOYER},
    entropy_api,
    error::EvoreError,
    instruction::MMStratAutodeploy,
    ore_api::{self, Board},
    processor::strategy_dispatch::{dispatch_strategy, StrategyResult},
    state::{Manager, StrategyDeployer},
};

pub fn process_mm_strat_autodeploy(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = MMStratAutodeploy::try_from_bytes(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);
    let amount = u64::from_le_bytes(args.amount);
    let squares_mask = u32::from_le_bytes(args.squares_mask);
    let extra = u32::from_le_bytes(args.extra);

    let [
        signer,
        manager_account_info,
        strat_deployer_account_info,
        managed_miner_auth_account_info,
        ore_miner_account_info,
        fee_collector_account_info,
        automation_account_info,
        config_account_info,
        board_account_info,
        round_account_info,
        entropy_var_account_info,
        ore_program,
        entropy_program,
        system_program_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if *ore_program.key != ore_api::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *system_program_info.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *entropy_program.key != entropy_api::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *fee_collector_account_info.key != FEE_COLLECTOR {
        return Err(EvoreError::InvalidFeeCollector.into());
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    let _manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    if strat_deployer_account_info.data_is_empty() {
        return Err(EvoreError::StratDeployerNotInitialized.into());
    }

    let (strat_deployer_pda, _) = Pubkey::find_program_address(
        &[STRATEGY_DEPLOYER, manager_account_info.key.as_ref()],
        &crate::id(),
    );

    if strat_deployer_pda != *strat_deployer_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let strat_deployer = strat_deployer_account_info.as_account::<StrategyDeployer>(&crate::id())?;
    let deploy_authority = strat_deployer.deploy_authority;
    let bps_fee = strat_deployer.bps_fee;
    let flat_fee = strat_deployer.flat_fee;
    let expected_bps_fee = strat_deployer.expected_bps_fee;
    let expected_flat_fee = strat_deployer.expected_flat_fee;
    let max_per_round = strat_deployer.max_per_round;
    let strategy_type = strat_deployer.strategy_type;
    let strategy_data = strat_deployer.strategy_data;

    let clock = Clock::get()?;
    let board = board_account_info.as_account::<Board>(&ore_api::id())?;

    if clock.slot >= board.end_slot {
        return Err(EvoreError::EndSlotReached.into());
    }

    let round = round_account_info.as_account::<ore_api::Round>(&ore_api::id())?;

    let StrategyResult { mut batches, total_to_deploy, needs_automation } = dispatch_strategy(
        strategy_type,
        &strategy_data,
        amount,
        squares_mask,
        extra,
        &board,
        &round,
        &clock,
    )?;

    if deploy_authority != *signer.key {
        return Err(EvoreError::InvalidDeployAuthority.into());
    }

    if expected_bps_fee > 0 && bps_fee > expected_bps_fee {
        return Err(EvoreError::UnexpectedFee.into());
    }
    if expected_flat_fee > 0 && flat_fee > expected_flat_fee {
        return Err(EvoreError::UnexpectedFee.into());
    }

    let (managed_miner_auth_pda, managed_miner_auth_bump) = Pubkey::find_program_address(
        &[MANAGED_MINER_AUTH, manager_account_info.key.as_ref(), &auth_id.to_le_bytes()],
        &crate::id(),
    );

    if managed_miner_auth_pda != *managed_miner_auth_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    if max_per_round > 0 {
        let already_deployed = if !ore_miner_account_info.data_is_empty() {
            let miner = ore_miner_account_info.as_account::<ore_api::Miner>(&ore_api::id())?;
            if miner.round_id == board.round_id {
                miner.deployed.iter().sum::<u64>()
            } else {
                0
            }
        } else {
            0
        };

        let total_for_round = already_deployed.saturating_add(total_to_deploy);
        if total_for_round > max_per_round {
            return Err(EvoreError::ExceedsMaxPerRound.into());
        }
    }

    let bps_fee_amount = if bps_fee > 0 {
        total_to_deploy.saturating_mul(bps_fee).saturating_div(10_000)
    } else {
        0
    };

    let deployer_fee = bps_fee_amount.saturating_add(flat_fee);
    let protocol_fee = DEPLOY_FEE;

    const AUTH_PDA_RENT: u64 = 890_880;
    let miner_rent = if ore_miner_account_info.data_is_empty() {
        let size = 8 + std::mem::size_of::<ore_api::Miner>();
        solana_program::rent::Rent::default().minimum_balance(size)
    } else {
        0
    };

    let automation_size = 8 + std::mem::size_of::<ore_api::Automation>();
    let automation_rent = if needs_automation {
        solana_program::rent::Rent::default().minimum_balance(automation_size)
    } else {
        0
    };

    let required_balance = AUTH_PDA_RENT
        .saturating_add(ore_api::CHECKPOINT_FEE)
        .saturating_add(total_to_deploy)
        .saturating_add(miner_rent)
        .saturating_add(deployer_fee)
        .saturating_add(protocol_fee)
        .saturating_add(automation_rent);

    let current_balance = managed_miner_auth_account_info.lamports();
    if current_balance < required_balance {
        return Err(EvoreError::InsufficientAutodeployBalance.into());
    }

    let auth_id_bytes = auth_id.to_le_bytes();
    let managed_miner_auth_seeds: &[&[u8]] = &[
        MANAGED_MINER_AUTH,
        manager_account_info.key.as_ref(),
        &auth_id_bytes,
        &[managed_miner_auth_bump],
    ];

    let is_already_deployed = if !ore_miner_account_info.data_is_empty() {
        let miner = ore_miner_account_info.as_account::<ore_api::Miner>(&ore_api::id())?;
        miner.round_id == board.round_id
    } else {
        false
    };

    if protocol_fee > 0 && !is_already_deployed {
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::transfer(
                managed_miner_auth_account_info.key,
                fee_collector_account_info.key,
                protocol_fee,
            ),
            &[
                managed_miner_auth_account_info.clone(),
                fee_collector_account_info.clone(),
                system_program_info.clone(),
            ],
            &[managed_miner_auth_seeds],
        )?;
    }

    if deployer_fee > 0 && !is_already_deployed {
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::transfer(
                managed_miner_auth_account_info.key,
                signer.key,
                deployer_fee,
            ),
            &[
                managed_miner_auth_account_info.clone(),
                signer.clone(),
                system_program_info.clone(),
            ],
            &[managed_miner_auth_seeds],
        )?;
    }

    let deploy_accounts = vec![
        managed_miner_auth_account_info.clone(),
        managed_miner_auth_account_info.clone(),
        automation_account_info.clone(),
        board_account_info.clone(),
        config_account_info.clone(),
        ore_miner_account_info.clone(),
        round_account_info.clone(),
        system_program_info.clone(),
        ore_program.clone(),
        entropy_var_account_info.clone(),
        entropy_program.clone(),
        ore_program.clone(),
    ];

    if needs_automation {
        batches.sort_by_key(|b| b.amount);
        let max_batch_amount = batches.last().map(|b| b.amount).unwrap_or(0);

        let deposit = managed_miner_auth_account_info.lamports()
            .saturating_sub(AUTH_PDA_RENT)
            .saturating_sub(automation_rent)
            .saturating_sub(miner_rent);

        solana_program::program::invoke_signed(
            &ore_api::automate(
                *managed_miner_auth_account_info.key,
                max_batch_amount,
                deposit,
                *managed_miner_auth_account_info.key,
                0, 0, 2, false,
            ),
            &[
                managed_miner_auth_account_info.clone(),
                automation_account_info.clone(),
                managed_miner_auth_account_info.clone(),
                ore_miner_account_info.clone(),
                system_program_info.clone(),
            ],
            &[managed_miner_auth_seeds],
        )?;

        for batch in &batches {
            if batch.amount == 0 {
                continue;
            }
            solana_program::program::invoke_signed(
                &ore_api::deploy(
                    *managed_miner_auth_account_info.key,
                    *managed_miner_auth_account_info.key,
                    batch.amount,
                    round.id,
                    batch.squares,
                ),
                &deploy_accounts,
                &[managed_miner_auth_seeds],
            )?;
        }

        if !automation_account_info.data_is_empty() {
            let close_ix = Instruction {
                program_id: ore_api::id(),
                accounts: vec![
                    AccountMeta::new(*managed_miner_auth_account_info.key, true),
                    AccountMeta::new(*automation_account_info.key, false),
                    AccountMeta::new_readonly(Pubkey::default(), false),
                    AccountMeta::new(*ore_miner_account_info.key, false),
                    AccountMeta::new_readonly(*system_program_info.key, false),
                ],
                data: ore_api::Automate {
                    amount: 0u64.to_le_bytes(),
                    deposit: 0u64.to_le_bytes(),
                    fee: 0u64.to_le_bytes(),
                    mask: 0u64.to_le_bytes(),
                    strategy: 0,
                    reload: 0u64.to_le_bytes(),
                }
                .to_bytes(),
            };
            solana_program::program::invoke_signed(
                &close_ix,
                &[
                    managed_miner_auth_account_info.clone(),
                    automation_account_info.clone(),
                    system_program_info.clone(),
                    ore_miner_account_info.clone(),
                    system_program_info.clone(),
                ],
                &[managed_miner_auth_seeds],
            )?;
        }
    } else {
        let batch = &batches[0];
        solana_program::program::invoke_signed(
            &ore_api::deploy(
                *managed_miner_auth_account_info.key,
                *managed_miner_auth_account_info.key,
                batch.amount,
                round.id,
                batch.squares,
            ),
            &deploy_accounts,
            &[managed_miner_auth_seeds],
        )?;
    }

    Ok(())
}
