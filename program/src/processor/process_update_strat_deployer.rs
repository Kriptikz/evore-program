use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey,
};
use steel::*;

use crate::{
    consts::STRATEGY_DEPLOYER,
    error::EvoreError,
    instruction::UpdateStratDeployer,
    state::{Manager, StrategyDeployer},
    validation::{StrategyType, validate_strategy_data},
};

pub fn process_update_strat_deployer(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = UpdateStratDeployer::try_from_bytes(instruction_data)?;
    let new_bps_fee = u64::from_le_bytes(args.bps_fee);
    let new_flat_fee = u64::from_le_bytes(args.flat_fee);
    let new_expected_bps_fee = u64::from_le_bytes(args.expected_bps_fee);
    let new_expected_flat_fee = u64::from_le_bytes(args.expected_flat_fee);
    let new_max_per_round = u64::from_le_bytes(args.max_per_round);
    let new_strategy_type = args.strategy_type;
    let new_strategy_data = args.strategy_data;

    let [
        signer,
        manager_account_info,
        strat_deployer_account_info,
        new_deploy_authority_info,
        _system_program_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    let manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    if strat_deployer_account_info.data_is_empty() {
        return Err(EvoreError::StratDeployerNotInitialized.into());
    }

    let (strat_deployer_pda, _bump) = Pubkey::find_program_address(
        &[STRATEGY_DEPLOYER, manager_account_info.key.as_ref()],
        &crate::id(),
    );

    if strat_deployer_pda != *strat_deployer_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let strat_deployer = strat_deployer_account_info.as_account::<StrategyDeployer>(&crate::id())?;
    let current_deploy_authority = strat_deployer.deploy_authority;

    let is_manager_authority = manager.authority == *signer.key;
    let is_deploy_authority = current_deploy_authority == *signer.key;

    if !is_manager_authority && !is_deploy_authority {
        return Err(EvoreError::NotAuthorized.into());
    }

    let mut data = strat_deployer_account_info.try_borrow_mut_data()?;

    if is_manager_authority {
        let strategy_type = StrategyType::try_from(new_strategy_type)?;
        validate_strategy_data(strategy_type, &new_strategy_data)?;

        data[40..72].copy_from_slice(new_deploy_authority_info.key.as_ref());
        data[88..96].copy_from_slice(&new_expected_bps_fee.to_le_bytes());
        data[96..104].copy_from_slice(&new_expected_flat_fee.to_le_bytes());
        data[104..112].copy_from_slice(&new_max_per_round.to_le_bytes());
        data[112..113].copy_from_slice(&[new_strategy_type]);
        data[113..177].copy_from_slice(&new_strategy_data);
    }

    if is_deploy_authority {
        data[40..72].copy_from_slice(new_deploy_authority_info.key.as_ref());
        data[72..80].copy_from_slice(&new_bps_fee.to_le_bytes());
        data[80..88].copy_from_slice(&new_flat_fee.to_le_bytes());
    }

    Ok(())
}
