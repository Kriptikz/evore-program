use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, system_program,
};
use steel::*;

use crate::{
    consts::STRATEGY_DEPLOYER,
    error::EvoreError,
    instruction::CreateStratDeployer,
    state::{EvoreAccount, Manager, StrategyDeployer},
    validation::{StrategyType, validate_strategy_data},
};

pub fn process_create_strat_deployer(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = CreateStratDeployer::try_from_bytes(instruction_data)?;
    let expected_bps_fee = u64::from_le_bytes(args.bps_fee);
    let expected_flat_fee = u64::from_le_bytes(args.flat_fee);
    let max_per_round = u64::from_le_bytes(args.max_per_round);
    let strategy_type_raw = args.strategy_type;
    let strategy_data = args.strategy_data;

    let [
        signer,
        manager_account_info,
        strat_deployer_account_info,
        deploy_authority_info,
        system_program_info,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if *system_program_info.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    let manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    if manager.authority != *signer.key {
        return Err(EvoreError::NotAuthorized.into());
    }

    if !strat_deployer_account_info.data_is_empty() {
        return Err(EvoreError::AlreadyInitialized.into());
    }

    let strategy_type = StrategyType::try_from(strategy_type_raw)?;
    validate_strategy_data(strategy_type, &strategy_data)?;

    let (strat_deployer_pda, strat_deployer_bump) = Pubkey::find_program_address(
        &[STRATEGY_DEPLOYER, manager_account_info.key.as_ref()],
        &crate::id(),
    );

    if strat_deployer_pda != *strat_deployer_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let account_size = 8 + std::mem::size_of::<StrategyDeployer>();
    let rent = solana_program::rent::Rent::get()?;
    let lamports = rent.minimum_balance(account_size);

    solana_program::program::invoke_signed(
        &solana_program::system_instruction::create_account(
            signer.key,
            strat_deployer_account_info.key,
            lamports,
            account_size as u64,
            &crate::id(),
        ),
        &[signer.clone(), strat_deployer_account_info.clone(), system_program_info.clone()],
        &[&[STRATEGY_DEPLOYER, manager_account_info.key.as_ref(), &[strat_deployer_bump]]],
    )?;

    let strat_deployer = StrategyDeployer {
        manager_key: *manager_account_info.key,
        deploy_authority: *deploy_authority_info.key,
        bps_fee: expected_bps_fee,
        flat_fee: expected_flat_fee,
        expected_bps_fee,
        expected_flat_fee,
        max_per_round,
        strategy_type: strategy_type_raw,
        strategy_data,
        _padding: [0u8; 7],
    };

    let mut data = strat_deployer_account_info.try_borrow_mut_data()?;
    let discr = (EvoreAccount::StrategyDeployer as u64).to_le_bytes();
    data[..8].copy_from_slice(&discr);
    data[8..8 + std::mem::size_of::<StrategyDeployer>()].copy_from_slice(strat_deployer.to_bytes());

    Ok(())
}
