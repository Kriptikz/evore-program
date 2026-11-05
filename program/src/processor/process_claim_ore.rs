use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use steel::*;

use crate::{
    error::EvoreError,
    instruction::MMClaimORE,
    ore_api::{self},
    state::Manager,
};

/// `auth_id` (8) + `bump` (1). This is the body every current client sends.
const MM_CLAIM_ORE_LEGACY_LEN: usize = 9;
/// Optional tail: Ore claim basis points, little-endian u64. 10_000 = 100%.
const MM_CLAIM_ORE_BPS_LEN: usize = 8;
/// A missing tail is a full claim. Ore uses the same 10_000 denominator.
const FULL_CLAIM_BPS: u64 = 10_000;

/// Parse an `MMClaimORE` body (discriminator already stripped).
///
/// 9 bytes keeps today's full claim. 17 bytes appends `bps` and forwards it to
/// Ore. `bps` is not a field of `MMClaimORE`: `try_from_bytes` requires an exact
/// length, and a `u64` after the `u8` bump would insert 7 bytes of padding.
pub(crate) fn parse_mm_claim_ore(
    instruction_data: &[u8],
) -> Result<(&MMClaimORE, u64), ProgramError> {
    let (head, bps) = if instruction_data.len() == MM_CLAIM_ORE_LEGACY_LEN {
        (instruction_data, FULL_CLAIM_BPS)
    } else if instruction_data.len() == MM_CLAIM_ORE_LEGACY_LEN + MM_CLAIM_ORE_BPS_LEN {
        let mut bps_bytes = [0u8; MM_CLAIM_ORE_BPS_LEN];
        bps_bytes.copy_from_slice(&instruction_data[MM_CLAIM_ORE_LEGACY_LEN..]);
        (
            &instruction_data[..MM_CLAIM_ORE_LEGACY_LEN],
            u64::from_le_bytes(bps_bytes),
        )
    } else {
        return Err(ProgramError::InvalidInstructionData);
    };

    Ok((MMClaimORE::try_from_bytes(head)?, bps))
}

pub fn process_claim_ore(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let (args, bps) = parse_mm_claim_ore(instruction_data)?;
    let auth_id = u64::from_le_bytes(args.auth_id);

    let [signer, manager_account_info, managed_miner_auth_account_info, board_account_info, ore_miner_account_info, mint_account_info, recipient_account_info, signer_recipient_account_info, treasury_account_info, treasury_tokens_account_info, system_program, spl_program, spl_ata_program, ore_program] =
        accounts
    else {
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

    if !recipient_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if !signer_recipient_account_info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    if manager_account_info.data_is_empty() {
        return Err(EvoreError::ManagerNotInitialized.into());
    }

    if *ore_program.key != ore_api::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *board_account_info.key != ore_api::board_pda().0 {
        return Err(EvoreError::InvalidPDA.into());
    }

    if *system_program.key != solana_program::system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *spl_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *spl_ata_program.key != spl_associated_token_account::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let manager = manager_account_info.as_account::<Manager>(&crate::id())?;

    if manager.authority != *signer.key {
        return Err(EvoreError::NotAuthorized.into());
    }

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

    let claim_ore_accounts = vec![
        managed_miner_auth_account_info.clone(),
        board_account_info.clone(),
        ore_miner_account_info.clone(),
        mint_account_info.clone(),
        recipient_account_info.clone(),
        treasury_account_info.clone(),
        treasury_tokens_account_info.clone(),
        system_program.clone(),
        spl_program.clone(),
        spl_ata_program.clone(),
        ore_program.clone(),
    ];
    let managed_miner_auth_key = claim_ore_accounts[0].key.clone();

    if recipient_account_info.data_is_empty() {
        create_associated_token_account(
            signer,
            managed_miner_auth_account_info,
            recipient_account_info,
            mint_account_info,
            system_program,
            spl_program,
            spl_ata_program,
        )?;
    } else {
        recipient_account_info.as_associated_token_account(
            managed_miner_auth_account_info.key,
            mint_account_info.key,
        )?;
    }

    solana_program::program::invoke_signed(
        &ore_api::claim_ore(managed_miner_auth_key, bps),
        &claim_ore_accounts,
        &[&[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
            &[args.bump],
        ]],
    )?;

    if signer_recipient_account_info.data_is_empty() {
        create_associated_token_account(
            signer,
            signer,
            signer_recipient_account_info,
            mint_account_info,
            system_program,
            spl_program,
            spl_ata_program,
        )?;
    } else {
        signer_recipient_account_info
            .as_associated_token_account(signer.key, mint_account_info.key)?;
    }

    let managed_auth_tokens = recipient_account_info
        .as_associated_token_account(&managed_miner_auth_key, &mint_account_info.key)?;

    transfer_signed_with_bump(
        managed_miner_auth_account_info,
        recipient_account_info,
        signer_recipient_account_info,
        spl_program,
        managed_auth_tokens.amount(),
        &[
            crate::consts::MANAGED_MINER_AUTH,
            manager_account_info.key.as_ref(),
            &auth_id.to_le_bytes(),
        ],
        args.bump,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{mm_claim_ore, mm_claim_ore_bps};
    use crate::ore_api::OreInstruction;

    fn body(ix_data: &[u8]) -> &[u8] {
        &ix_data[1..]
    }

    #[test]
    fn legacy_claim_is_ten_bytes_and_parses_as_full_claim() {
        let signer = Pubkey::new_unique();
        let manager = Pubkey::new_unique();
        let ix = mm_claim_ore(signer, manager, 7);
        assert_eq!(ix.data.len(), 10);
        assert_eq!(ix.data[0], 4);

        let (args, bps) = parse_mm_claim_ore(body(&ix.data)).unwrap();
        assert_eq!(bps, FULL_CLAIM_BPS);
        assert_eq!(u64::from_le_bytes(args.auth_id), 7);
    }

    #[test]
    fn bps_tail_is_eighteen_bytes_and_round_trips() {
        let signer = Pubkey::new_unique();
        let manager = Pubkey::new_unique();
        let legacy = mm_claim_ore(signer, manager, 7);
        let ix = mm_claim_ore_bps(signer, manager, 7, 2_500);

        assert_eq!(ix.data.len(), 18);
        assert_eq!(&ix.data[..10], &legacy.data[..]);
        assert_eq!(
            u64::from_le_bytes(ix.data[10..18].try_into().unwrap()),
            2_500
        );
        assert_eq!(ix.accounts, legacy.accounts);

        let (args, bps) = parse_mm_claim_ore(body(&ix.data)).unwrap();
        assert_eq!(bps, 2_500);
        assert_eq!(u64::from_le_bytes(args.auth_id), 7);
    }

    #[test]
    fn zero_bps_is_not_rewritten_to_a_full_claim() {
        let ix = mm_claim_ore_bps(Pubkey::new_unique(), Pubkey::new_unique(), 1, 0);
        let (_, bps) = parse_mm_claim_ore(body(&ix.data)).unwrap();
        assert_eq!(bps, 0);
    }

    #[test]
    fn bps_above_the_denominator_is_forwarded_unchanged() {
        let ix = mm_claim_ore_bps(Pubkey::new_unique(), Pubkey::new_unique(), 1, 10_001);
        let (_, bps) = parse_mm_claim_ore(body(&ix.data)).unwrap();
        assert_eq!(bps, 10_001);

        let cpi = ore_api::claim_ore(Pubkey::new_unique(), bps);
        assert_eq!(cpi.data.len(), 9);
        assert_eq!(cpi.data[0], OreInstruction::ClaimORE as u8);
        assert_eq!(
            u64::from_le_bytes(cpi.data[1..9].try_into().unwrap()),
            10_001
        );
    }

    #[test]
    fn other_body_lengths_are_rejected() {
        for len in [0usize, 8, 10, 16, 18] {
            let err = parse_mm_claim_ore(&vec![0u8; len]).unwrap_err();
            assert_eq!(err, ProgramError::InvalidInstructionData);
        }
    }
}
