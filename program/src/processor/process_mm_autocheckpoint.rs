use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, system_program,
};
use steel::*;

use crate::{
    consts::{DEPLOYER, MANAGED_MINER_AUTH},
    error::EvoreError,
    instruction::MMAutocheckpoint,
    ore_api::{self},
    state::{Deployer, Manager},
};

/// Process MMAutocheckpoint instruction
///
/// Similar to MMCheckpoint but can be called by deploy_authority instead of manager authority.
/// This allows the autodeploy crank to checkpoint before deploying.
pub fn process_mm_autocheckpoint(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = MMAutocheckpoint::try_from_bytes(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);

    let (
        signer,
        manager_account_info,
        deployer_account_info,
        managed_miner_auth_account_info,
        ore_miner_account_info,
        treasury_account_info,
        board_account_info,
        round_account_info,
        system_program_info,
        ore_program,
        automation_account_info,
    ) = match accounts {
        [signer, manager_account_info, deployer_account_info, managed_miner_auth_account_info, ore_miner_account_info, treasury_account_info, board_account_info, round_account_info, system_program_info, ore_program] => {
            (
                signer,
                manager_account_info,
                deployer_account_info,
                managed_miner_auth_account_info,
                ore_miner_account_info,
                treasury_account_info,
                board_account_info,
                round_account_info,
                system_program_info,
                ore_program,
                None,
            )
        }
        [signer, manager_account_info, deployer_account_info, managed_miner_auth_account_info, ore_miner_account_info, treasury_account_info, board_account_info, round_account_info, system_program_info, ore_program, automation_account_info] => {
            (
                signer,
                manager_account_info,
                deployer_account_info,
                managed_miner_auth_account_info,
                ore_miner_account_info,
                treasury_account_info,
                board_account_info,
                round_account_info,
                system_program_info,
                ore_program,
                Some(automation_account_info),
            )
        }
        _ => return Err(ProgramError::NotEnoughAccountKeys),
    };

    // Basic validations
    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !managed_miner_auth_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if *ore_program.key != ore_api::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *system_program_info.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Verify manager is initialized
    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    let _manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    // Verify deployer is initialized
    if deployer_account_info.data_is_empty() {
        return Err(EvoreError::DeployerNotInitialized.into());
    }

    // Verify deployer PDA
    let (deployer_pda, _deployer_bump) =
        Pubkey::find_program_address(&[DEPLOYER, manager_account_info.key.as_ref()], &crate::id());

    if deployer_pda != *deployer_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    // Load deployer and verify signer is deploy_authority
    let deployer = deployer_account_info.as_account::<Deployer>(&crate::id())?;

    if deployer.deploy_authority != *signer.key {
        return Err(EvoreError::InvalidDeployAuthority.into());
    }

    // Verify managed_miner_auth PDA
    let managed_miner_auth_pda = Pubkey::create_program_address(
        &[
            MANAGED_MINER_AUTH,
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

    let checkpoint_round_id = if ore_miner_account_info.data_is_empty() {
        return Err(ProgramError::InvalidAccountData);
    } else {
        let ore_miner = ore_api::load_miner(ore_miner_account_info)?;
        ore_miner.round_id()
    };

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
                ore_api::checkpoint_v2(
                    managed_miner_auth_key,
                    managed_miner_auth_key,
                    checkpoint_round_id,
                ),
                vec![
                    managed_miner_auth_account_info.clone(),
                    managed_miner_auth_account_info.clone(),
                    automation_account_info.clone(),
                    board_account_info.clone(),
                    ore_miner_account_info.clone(),
                    round_account_info.clone(),
                    treasury_account_info.clone(),
                    system_program_info.clone(),
                    ore_program.clone(),
                ],
            )
        } else {
            (
                ore_api::checkpoint(
                    managed_miner_auth_key,
                    managed_miner_auth_key,
                    checkpoint_round_id,
                ),
                vec![
                    managed_miner_auth_account_info.clone(),
                    board_account_info.clone(),
                    ore_miner_account_info.clone(),
                    round_account_info.clone(),
                    treasury_account_info.clone(),
                    system_program_info.clone(),
                    ore_program.clone(),
                ],
            )
        };

    // Call ORE checkpoint CPI
    solana_program::program::invoke_signed(
        &checkpoint_instruction,
        &checkpoint_accounts,
        &[&[
            MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ]],
    )?;

    Ok(())
}
