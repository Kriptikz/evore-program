use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey,
};
use steel::*;

use crate::{
    consts::{STRATEGY_DEPLOYER, MANAGED_MINER_AUTH},
    error::EvoreError,
    instruction::MMStratAutocheckpoint,
    ore_api::{self, Miner},
    state::{StrategyDeployer, Manager},
};

pub fn process_mm_strat_autocheckpoint(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = MMStratAutocheckpoint::try_from_bytes(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);

    let [
        signer,
        manager_account_info,
        strat_deployer_account_info,
        managed_miner_auth_account_info,
        ore_miner_account_info,
        ore_program,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if *ore_program.key != ore_api::id() {
        return Err(ProgramError::IncorrectProgramId);
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

    if strat_deployer.deploy_authority != *signer.key {
        return Err(EvoreError::InvalidDeployAuthority.into());
    }

    let managed_miner_auth_pda = Pubkey::create_program_address(
        &[
            MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ],
        &crate::id(),
    ).map_err(|_| EvoreError::InvalidPDA)?;

    if managed_miner_auth_pda != *managed_miner_auth_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let expected_ore_miner = ore_api::miner_pda(*managed_miner_auth_account_info.key).0;
    if expected_ore_miner != *ore_miner_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    if ore_miner_account_info.data_is_empty() {
        return Err(ProgramError::InvalidAccountData);
    }

    let _ore_miner = ore_miner_account_info.as_account::<Miner>(&ore_api::id())?;

    Ok(())
}
