mod strat_common;

use strat_common::*;

use evore::state::{strategy_deployer_pda, managed_miner_auth_pda};
use evore::instruction::{create_strat_deployer, mm_strat_autodeploy};
use solana_sdk::{signature::Keypair, signer::Signer, pubkey::Pubkey};

async fn setup_dynev_test(
    max_per_square: u64,
    min_bet: u64,
    slots_left: u64,
    max_ore_value: u64,
) -> (
    solana_program_test::ProgramTestContext,
    Keypair,
    Pubkey,
    Pubkey,
    u64,
) {
    let mut program_test = setup_programs();
    let manager = Keypair::new();
    let authority = Keypair::new();
    let deploy_authority = Keypair::new();
    let auth_id: u64 = 0;

    add_manager_account(&mut program_test, manager.pubkey(), authority.pubkey());

    let (mma_pda, _) = managed_miner_auth_pda(manager.pubkey(), auth_id);

    setup_strat_deploy_test_accounts(&mut program_test, 0, 1, 500);
    add_autodeploy_balance(&mut program_test, mma_pda, 50_000_000_000);

    let strategy_data = dynev_strategy_data(max_per_square, min_bet, slots_left, max_ore_value);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();

    let fund_ix = solana_sdk::system_instruction::transfer(&payer.pubkey(), &authority.pubkey(), 2_000_000_000);
    let fund_ix2 = solana_sdk::system_instruction::transfer(&payer.pubkey(), &deploy_authority.pubkey(), 2_000_000_000);
    let fund_fc = solana_sdk::system_instruction::transfer(&payer.pubkey(), &evore::consts::FEE_COLLECTOR, 1_000_000_000);
    send_transaction(&mut context, &[fund_ix, fund_ix2, fund_fc], &[&payer]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    let ix = create_strat_deployer(
        authority.pubkey(), manager.pubkey(), deploy_authority.pubkey(),
        0, 0, 0,
        5, // DynamicEv
        strategy_data,
    );
    send_transaction(&mut context, &[ix], &[&payer, &authority]).await.unwrap();
    context.last_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();

    (context, deploy_authority, manager.pubkey(), mma_pda, auth_id)
}

#[tokio::test]
async fn test_dynev_deploys_with_instruction_ore_value() {
    let (mut context, deploy_authority, manager, _, auth_id) =
        setup_dynev_test(
            500_000_000,  // max 0.5 SOL per square
            1_000_000,    // min bet 0.001 SOL
            500,          // slots_left
            5_000_000_000, // max 5 SOL ore value
        ).await;
    let payer = context.payer.insecure_clone();

    // DynEV: amount = bankroll, squares_mask:extra = ore_value as u64
    // ore_value passed as two u32 halves: low in squares_mask, high in extra
    let bankroll: u64 = 3_000_000_000;
    let ore_value: u64 = 2_000_000_000; // 2 SOL, within max
    let ore_value_low = (ore_value & 0xFFFFFFFF) as u32;
    let ore_value_high = ((ore_value >> 32) & 0xFFFFFFFF) as u32;

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(), manager, auth_id,
        bankroll,
        ore_value_low,
        ore_value_high,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_ok(), "DynEV autodeploy should succeed: {:?}", result.err());
}

#[tokio::test]
async fn test_dynev_rejects_ore_value_above_max() {
    let (mut context, deploy_authority, manager, _, auth_id) =
        setup_dynev_test(
            500_000_000,
            1_000_000,
            500,
            1_000_000_000, // max 1 SOL ore value
        ).await;
    let payer = context.payer.insecure_clone();

    // Try to pass ore_value = 2 SOL > max 1 SOL
    let ore_value: u64 = 2_000_000_000;
    let ore_value_low = (ore_value & 0xFFFFFFFF) as u32;
    let ore_value_high = ((ore_value >> 32) & 0xFFFFFFFF) as u32;

    let ix = mm_strat_autodeploy(
        deploy_authority.pubkey(), manager, auth_id,
        3_000_000_000,
        ore_value_low,
        ore_value_high,
    );

    let result = send_transaction(&mut context, &[ix], &[&payer, &deploy_authority]).await;
    assert!(result.is_err(), "DynEV should reject ore_value above max");
}
