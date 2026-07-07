use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, system_program,
};
use steel::*;

use crate::{
    error::EvoreError,
    instruction::MMCheckpoint,
    ore_api::{self},
    state::Manager,
};

pub fn process_checkpoint(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = MMCheckpoint::try_from_bytes(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);

    let (
        signer,
        manager_account_info,
        managed_miner_auth_account_info,
        ore_miner_account_info,
        treasury_account_info,
        board_account_info,
        round_account_info,
        system_program,
        ore_program,
        automation_account_info,
    ) = match accounts {
        [signer, manager_account_info, managed_miner_auth_account_info, ore_miner_account_info, treasury_account_info, board_account_info, round_account_info, system_program, ore_program] => {
            (
                signer,
                manager_account_info,
                managed_miner_auth_account_info,
                ore_miner_account_info,
                treasury_account_info,
                board_account_info,
                round_account_info,
                system_program,
                ore_program,
                None,
            )
        }
        [signer, manager_account_info, managed_miner_auth_account_info, ore_miner_account_info, treasury_account_info, board_account_info, round_account_info, system_program, ore_program, automation_account_info] => {
            (
                signer,
                manager_account_info,
                managed_miner_auth_account_info,
                ore_miner_account_info,
                treasury_account_info,
                board_account_info,
                round_account_info,
                system_program,
                ore_program,
                Some(automation_account_info),
            )
        }
        _ => return Err(ProgramError::NotEnoughAccountKeys),
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !manager_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if !managed_miner_auth_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    if *ore_program.key != ore_api::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *system_program.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    if manager.authority != *signer.key {
        return Err(EvoreError::NotAuthorized.into());
    }

    let round = ore_api::load_round(round_account_info)?;

    // Use create_program_address with bump from instruction data for deterministic CU usage
    let managed_miner_auth_pda = Pubkey::create_program_address(
        &[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ],
        &crate::id(),
    )
    .map_err(|_| EvoreError::InvalidPDA)?;

    if managed_miner_auth_pda != *managed_miner_auth_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let managed_miner_auth_key = *managed_miner_auth_account_info.key;

    let (checkpoint_instruction, checkpoint_accounts) =
        if let Some(automation_account_info) = automation_account_info {
            automation_account_info.has_seeds(
                &[
                    ore_api::AUTOMATION,
                    managed_miner_auth_account_info.key.as_ref(),
                ],
                &ore_api::id(),
            )?;

            (
                ore_api::checkpoint_v2(managed_miner_auth_key, managed_miner_auth_key, round.id()),
                vec![
                    managed_miner_auth_account_info.clone(),
                    managed_miner_auth_account_info.clone(),
                    automation_account_info.clone(),
                    board_account_info.clone(),
                    ore_miner_account_info.clone(),
                    round_account_info.clone(),
                    treasury_account_info.clone(),
                    system_program.clone(),
                    ore_program.clone(),
                ],
            )
        } else {
            (
                ore_api::checkpoint(managed_miner_auth_key, managed_miner_auth_key, round.id()),
                vec![
                    managed_miner_auth_account_info.clone(),
                    board_account_info.clone(),
                    ore_miner_account_info.clone(),
                    round_account_info.clone(),
                    treasury_account_info.clone(),
                    system_program.clone(),
                    ore_program.clone(),
                ],
            )
        };

    solana_program::program::invoke_signed(
        &checkpoint_instruction,
        &checkpoint_accounts,
        &[&[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ]],
    )?;

    Ok(())
}
