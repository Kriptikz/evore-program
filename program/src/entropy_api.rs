use solana_program::pubkey;
use steel::*;

pub const PROGRAM_ID: Pubkey = pubkey!("3jSkUuYBoJzQPMEzTvkDFXCZUBksPamrVhrnHR9igu2X");


/// Seed of the var account PDA.
pub const VAR: &[u8] = b"var";

/// Fetch PDA of the var account.
pub fn var_pda(authority: Pubkey, id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[VAR, &authority.to_bytes(), &id.to_le_bytes()],
        &PROGRAM_ID,
    )
}

pub fn id() -> Pubkey {
    PROGRAM_ID
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum EntropyInstruction {
    Open = 0,
    Close = 1,
    Next = 2,
    Reveal = 4,
    Sample = 5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Open {
    /// The id of the variable.
    pub id: [u8; 8],

    /// The commit provided by Entropy provider.
    pub commit: [u8; 32],

    /// Whether or not the Entropy provider should automatically sample the slot hash.
    pub is_auto: [u8; 8],

    /// The number of random variables to sample.
    pub samples: [u8; 8],

    /// The slot at which the variable should sample the slothash.
    pub end_at: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Close {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Next {
    pub end_at: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Reveal {
    pub seed: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Sample {}

instruction!(EntropyInstruction, Open);
instruction!(EntropyInstruction, Close);
instruction!(EntropyInstruction, Next);
instruction!(EntropyInstruction, Reveal);
instruction!(EntropyInstruction, Sample);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum EntropyAccount {
    Var = 0,
}


#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Var {
    /// The creator of the variable.
    pub authority: Pubkey,

    /// The id of the variable.
    pub id: u64,

    /// The provider of the entropy data.
    pub provider: Pubkey,

    /// The commit provided by Entropy provider.
    pub commit: [u8; 32],

    /// The revealed seed.
    pub seed: [u8; 32],

    /// The slot hash
    pub slot_hash: [u8; 32],

    /// The current value of the variable.
    pub value: [u8; 32],

    /// The number of random variables remaining to be sampled.
    pub samples: u64,

    /// Whether or not the Entropy provider should automatically sample the slot hash.
    pub is_auto: u64,

    /// The slot at which the variable was opened.
    pub start_at: u64,

    /// The slot at which the variable should sample the slothash.
    pub end_at: u64,
}

account!(EntropyAccount, Var);


pub fn next(signer: Pubkey, var: Pubkey, end_at: u64) -> Instruction {
  Instruction {
      program_id: PROGRAM_ID,
      accounts: vec![AccountMeta::new(signer, true), AccountMeta::new(var, false)],
      data: Next {
          end_at: end_at.to_le_bytes(),
      }
      .to_bytes(),
  }
}

pub fn reveal(signer: Pubkey, var: Pubkey, seed: [u8; 32]) -> Instruction {
  Instruction {
      program_id: PROGRAM_ID,
      accounts: vec![AccountMeta::new(signer, true), AccountMeta::new(var, false)],
      data: Reveal { seed }.to_bytes(),
  }
}

pub fn sample(signer: Pubkey, var: Pubkey) -> Instruction {
  Instruction {
      program_id: PROGRAM_ID,
      accounts: vec![
          AccountMeta::new(signer, true),
          AccountMeta::new(var, false),
          AccountMeta::new_readonly(sysvar::slot_hashes::ID, false),
      ],
      data: Sample {}.to_bytes(),
  }
}
