use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, system_program
};
use steel::*;

use crate::{
    error::EvoreError, instruction::WithdrawTokens, state::Manager
};

pub fn process_withdraw_tokens(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let args = WithdrawTokens::try_from_bytes(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);

    let [
        signer,
        manager_account_info,
        managed_miner_auth_account_info,
        source_ata_account_info,
        destination_ata_account_info,
        mint_account_info,
        system_program,
        spl_program,
        spl_ata_program,
    ] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !signer.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if !managed_miner_auth_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if !source_ata_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if !destination_ata_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    if *system_program.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *spl_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *spl_ata_program.key != spl_associated_token_account::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let manager = manager_account_info
        .as_account::<Manager>(&crate::id())?;

    if manager.authority != *signer.key {
        return Err(EvoreError::NotAuthorized.into());
    }

    let managed_miner_auth_pda = Pubkey::create_program_address(
        &[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ],
        &crate::id(),
    ).map_err(|_| EvoreError::InvalidPDA)?;

    if managed_miner_auth_pda != *managed_miner_auth_account_info.key {
        return Err(EvoreError::InvalidPDA.into());
    }

    let source_tokens = source_ata_account_info
        .as_associated_token_account(managed_miner_auth_account_info.key, mint_account_info.key)?;

    if destination_ata_account_info.data_is_empty() {
        create_associated_token_account(
            signer,
            signer,
            destination_ata_account_info,
            mint_account_info,
            system_program,
            spl_program,
            spl_ata_program,
        )?;
    } else {
        destination_ata_account_info
            .as_associated_token_account(signer.key, mint_account_info.key)?;
    }

    transfer_signed_with_bump(
        managed_miner_auth_account_info,
        source_ata_account_info,
        destination_ata_account_info,
        spl_program,
        source_tokens.amount(),
        &[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
        ],
        args.bump,
    )?;

    Ok(())
}
