use evore::{
    consts::FEE_COLLECTOR,
    entropy_api::{self, var_pda, Var},
    ore_api::{
        self, board_pda, config_pda, miner_pda, round_pda,
        Board, Miner, Round, MINT_ADDRESS, TREASURY_ADDRESS,
    },
    state::{managed_miner_auth_pda, EvoreAccount, Manager, StrategyDeployer, strategy_deployer_pda},
};
use solana_program::rent::Rent;
use solana_program_test::{processor, read_file, ProgramTest};
use solana_sdk::{
    account::Account, compute_budget::ComputeBudgetInstruction,
    pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction,
};
use steel::{AccountDeserialize, Numeric};

pub const TEST_ROUND_ID: u64 = 70149;

// ============================================================================
// Program Setup
// ============================================================================

pub fn setup_programs() -> ProgramTest {
    let mut program_test = ProgramTest::new(
        "evore",
        evore::id(),
        processor!(evore::process_instruction),
    );

    let data = read_file(&"tests/buffers/oreV3.so");
    program_test.add_account(
        ore_api::id(),
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    let data = read_file(&"tests/buffers/entropy.so");
    program_test.add_account(
        entropy_api::id(),
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    program_test
}

// ============================================================================
// Evore Account Helpers
// ============================================================================

pub fn add_manager_account(
    program_test: &mut ProgramTest,
    manager_address: Pubkey,
    authority: Pubkey,
) {
    let manager = Manager { authority };

    let mut data = Vec::new();
    let discr = (EvoreAccount::Manager as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(manager.to_bytes());

    program_test.add_account(
        manager_address,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: evore::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_strat_deployer_account(
    program_test: &mut ProgramTest,
    strat_deployer_address: Pubkey,
    manager_key: Pubkey,
    deploy_authority: Pubkey,
    bps_fee: u64,
    flat_fee: u64,
    expected_bps_fee: u64,
    expected_flat_fee: u64,
    max_per_round: u64,
    strategy_type: u8,
    strategy_data: [u8; 64],
) {
    let strat_deployer = StrategyDeployer {
        manager_key,
        deploy_authority,
        bps_fee,
        flat_fee,
        expected_bps_fee,
        expected_flat_fee,
        max_per_round,
        strategy_type,
        strategy_data,
        _padding: [0u8; 7],
    };

    let mut data = Vec::new();
    let discr = (EvoreAccount::StrategyDeployer as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(strat_deployer.to_bytes());

    program_test.add_account(
        strat_deployer_address,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: evore::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn fund_account(program_test: &mut ProgramTest, pubkey: Pubkey, lamports: u64) {
    program_test.add_account(
        pubkey,
        Account {
            lamports,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_autodeploy_balance(
    program_test: &mut ProgramTest,
    managed_miner_auth_address: Pubkey,
    lamports: u64,
) {
    program_test.add_account(
        managed_miner_auth_address,
        Account {
            lamports,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

// ============================================================================
// ORE Account Helpers
// ============================================================================

pub fn add_board_account(
    program_test: &mut ProgramTest,
    round_id: u64,
    start_slot: u64,
    end_slot: u64,
    epoch_id: u64,
) -> Board {
    let board = Board {
        round_id,
        start_slot,
        end_slot,
        epoch_id,
    };

    let mut data = Vec::new();
    let discr = (ore_api::OreAccount::Board as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(board.to_bytes());

    program_test.add_account(
        board_pda().0,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: ore_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    board
}

pub fn add_round_account(
    program_test: &mut ProgramTest,
    round_id: u64,
    deployed: [u64; 25],
    total_deployed: u64,
    expires_at: u64,
) {
    let round = Round {
        id: round_id,
        deployed,
        slot_hash: [0u8; 32],
        count: [0u64; 25],
        expires_at,
        motherlode: 0,
        rent_payer: Pubkey::default(),
        top_miner: Pubkey::default(),
        top_miner_reward: 0,
        total_deployed,
        total_miners: 0,
        total_vaulted: 0,
        total_winnings: 0,
    };

    let mut data = Vec::new();
    let discr = (ore_api::OreAccount::Round as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(round.to_bytes());

    program_test.add_account(
        round_pda(round_id).0,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: ore_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_ore_miner_account(
    program_test: &mut ProgramTest,
    authority: Pubkey,
    deployed: [u64; 25],
    rewards_sol: u64,
    rewards_ore: u64,
    checkpoint_id: u64,
    round_id: u64,
) {
    let miner = Miner {
        authority,
        deployed,
        cumulative: [0; 25],
        checkpoint_fee: 10000,
        checkpoint_id,
        last_claim_ore_at: 0,
        last_claim_sol_at: 0,
        rewards_factor: Numeric::ZERO,
        rewards_sol,
        rewards_ore,
        refined_ore: 0,
        round_id,
        lifetime_rewards_sol: 0,
        lifetime_rewards_ore: 0,
        lifetime_deployed: 0,
    };

    let mut data = Vec::new();
    let discr = (ore_api::OreAccount::Miner as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(miner.to_bytes());

    program_test.add_account(
        miner_pda(authority).0,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1) + rewards_sol,
            data,
            owner: ore_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_entropy_var_account(
    program_test: &mut ProgramTest,
    board_address: Pubkey,
    end_at: u64,
) {
    let var = Var {
        authority: board_address,
        id: 0,
        provider: Pubkey::default(),
        commit: [0u8; 32],
        seed: [0u8; 32],
        slot_hash: [0u8; 32],
        value: [0u8; 32],
        samples: 0,
        is_auto: 0,
        start_at: 0,
        end_at,
    };

    let mut data = Vec::new();
    let discr = (entropy_api::EntropyAccount::Var as u64).to_le_bytes();
    data.extend_from_slice(&discr);
    data.extend_from_slice(var.to_bytes());

    program_test.add_account(
        var_pda(board_address, 0).0,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: entropy_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_treasury_account(program_test: &mut ProgramTest) {
    let data = read_file(&"tests/buffers/treasury_account.so");
    program_test.add_account(
        TREASURY_ADDRESS,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: ore_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_mint_account(program_test: &mut ProgramTest) {
    let data = read_file(&"tests/buffers/mint_account.so");
    program_test.add_account(
        MINT_ADDRESS,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: spl_token::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_treasury_ata_account(program_test: &mut ProgramTest) {
    let data = read_file(&"tests/buffers/treasury_at_account.so");
    program_test.add_account(
        spl_associated_token_account::get_associated_token_address(
            &TREASURY_ADDRESS,
            &MINT_ADDRESS,
        ),
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: spl_token::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn add_config_account(program_test: &mut ProgramTest) {
    let data = read_file(&"tests/buffers/config_account.so");
    program_test.add_account(
        config_pda().0,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: ore_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

// ============================================================================
// Deploy Test Setup
// ============================================================================

pub fn setup_strat_deploy_test_accounts(
    program_test: &mut ProgramTest,
    round_id: u64,
    current_slot: u64,
    slots_until_end: u64,
) -> Board {
    let end_slot = current_slot + slots_until_end;

    let board = add_board_account(program_test, round_id, current_slot, end_slot, 0);

    let mut deployed = [0u64; 25];
    deployed[0] = 3_000_000_000;
    deployed[1] = 2_500_000_000;
    deployed[2] = 2_000_000_000;
    deployed[3] = 1_500_000_000;
    deployed[4] = 1_000_000_000;
    deployed[5] = 800_000_000;
    deployed[6] = 600_000_000;
    deployed[7] = 500_000_000;
    deployed[8] = 200_000_000;
    deployed[9] = 200_000_000;
    deployed[10] = 100_000_000;
    let total_deployed: u64 = deployed.iter().sum();
    add_round_account(program_test, round_id, deployed, total_deployed, end_slot + 1000);

    add_entropy_var_account(program_test, board_pda().0, end_slot);
    add_treasury_account(program_test);
    add_mint_account(program_test);
    add_treasury_ata_account(program_test);
    add_config_account(program_test);

    board
}

// ============================================================================
// Strategy Data Builders
// ============================================================================

pub fn ev_strategy_data(max_per_square: u64, min_bet: u64, slots_left: u64, ore_value: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&max_per_square.to_le_bytes());
    d[8..16].copy_from_slice(&min_bet.to_le_bytes());
    d[16..24].copy_from_slice(&slots_left.to_le_bytes());
    d[24..32].copy_from_slice(&ore_value.to_le_bytes());
    d
}

pub fn percentage_strategy_data(percentage: u64, squares_count: u64, motherlode_min: u64, motherlode_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&percentage.to_le_bytes());
    d[8..16].copy_from_slice(&squares_count.to_le_bytes());
    d[16..24].copy_from_slice(&motherlode_min.to_le_bytes());
    d[24..32].copy_from_slice(&motherlode_max.to_le_bytes());
    d
}

pub fn manual_strategy_data() -> [u8; 64] {
    [0u8; 64]
}

pub fn split_strategy_data(motherlode_min: u64, motherlode_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&motherlode_min.to_le_bytes());
    d[8..16].copy_from_slice(&motherlode_max.to_le_bytes());
    d
}

pub fn dsp_strategy_data(percentage: u64, squares_mask: u64, motherlode_min: u64, motherlode_max: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&percentage.to_le_bytes());
    d[8..16].copy_from_slice(&squares_mask.to_le_bytes());
    d[16..24].copy_from_slice(&motherlode_min.to_le_bytes());
    d[24..32].copy_from_slice(&motherlode_max.to_le_bytes());
    d
}

pub fn dynev_strategy_data(max_per_square: u64, min_bet: u64, slots_left: u64, max_ore_value: u64) -> [u8; 64] {
    let mut d = [0u8; 64];
    d[0..8].copy_from_slice(&max_per_square.to_le_bytes());
    d[8..16].copy_from_slice(&min_bet.to_le_bytes());
    d[16..24].copy_from_slice(&slots_left.to_le_bytes());
    d[24..32].copy_from_slice(&max_ore_value.to_le_bytes());
    d
}

// ============================================================================
// State Helpers
// ============================================================================

pub async fn get_strat_deployer_state(
    banks_client: &mut solana_program_test::BanksClient,
    address: Pubkey,
) -> StrategyDeployer {
    let account = banks_client.get_account(address).await.unwrap().unwrap();
    *StrategyDeployer::try_from_bytes(&account.data).unwrap()
}

// ============================================================================
// Transaction Helpers
// ============================================================================

pub async fn send_transaction(
    context: &mut solana_program_test::ProgramTestContext,
    instructions: &[solana_program::instruction::Instruction],
    signers: &[&Keypair],
) -> Result<(), solana_program_test::BanksClientError> {
    let mut all_ixs = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(400_000),
    ];
    all_ixs.extend_from_slice(instructions);

    let tx = Transaction::new_signed_with_payer(
        &all_ixs,
        Some(&context.payer.pubkey()),
        signers,
        context.last_blockhash,
    );

    context.banks_client.process_transaction(tx).await
}
