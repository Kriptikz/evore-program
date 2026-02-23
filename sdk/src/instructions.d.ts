import { PublicKey, TransactionInstruction } from "@solana/web3.js";

// Manager
export declare function createManagerInstruction(signer: PublicKey, managerAccount: PublicKey): TransactionInstruction;
export declare function transferManagerInstruction(signer: PublicKey, manager: PublicKey, newAuthority: PublicKey): TransactionInstruction;

// Deploy (manager authority)
export declare function evDeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  bankroll: bigint,
  maxPerSquare: bigint,
  minBet: bigint,
  oreValue: bigint,
  slotsLeft: bigint,
  attempts: bigint,
  allowMultiDeploy?: boolean
): TransactionInstruction;

export declare function percentageDeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  bankroll: bigint,
  percentage: bigint,
  squaresCount: bigint,
  allowMultiDeploy?: boolean
): TransactionInstruction;

export declare function manualDeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  amounts: bigint[],
  allowMultiDeploy?: boolean
): TransactionInstruction;

export declare function splitDeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  amount: bigint,
  allowMultiDeploy?: boolean
): TransactionInstruction;

// Checkpoint & Claim (manager authority)
export declare function mmCheckpointInstruction(
  signer: PublicKey,
  manager: PublicKey,
  roundId: bigint,
  authId?: bigint
): TransactionInstruction;

export declare function mmClaimSolInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId?: bigint
): TransactionInstruction;

export declare function mmClaimOreInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId?: bigint
): TransactionInstruction;

// Deployer (manager authority creates, both can update)
// - Manager sets: expectedBpsFee, expectedFlatFee (max fees they accept), maxPerRound
// - Deploy authority sets: bpsFee, flatFee (actual fees charged, must be <= expected)
export declare function createDeployerInstruction(
  signer: PublicKey,
  manager: PublicKey,
  deployAuthority: PublicKey,
  /** Max bps fee user accepts (deployer can charge up to this) */
  bpsFee: bigint,
  /** Max flat fee user accepts (deployer can charge up to this) */
  flatFee?: bigint,
  maxPerRound?: bigint
): TransactionInstruction;

export declare function updateDeployerInstruction(
  signer: PublicKey,
  manager: PublicKey,
  newDeployAuthority: PublicKey,
  /** Actual bps fee (deploy authority only, must be <= expected) */
  newBpsFee: bigint,
  /** Actual flat fee (deploy authority only, must be <= expected) */
  newFlatFee?: bigint,
  /** Max bps fee user accepts (manager only) */
  newExpectedBpsFee?: bigint,
  /** Max flat fee user accepts (manager only) */
  newExpectedFlatFee?: bigint,
  newMaxPerRound?: bigint
): TransactionInstruction;

// Autodeploy Balance (manager authority)
export declare function depositAutodeployBalanceInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  amount: bigint
): TransactionInstruction;

export declare function withdrawAutodeployBalanceInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  amount: bigint
): TransactionInstruction;

// Autodeploy (deploy authority - for executors)
export declare function mmAutodeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  amount: bigint,
  squaresMask: number
): TransactionInstruction;

export declare function mmAutocheckpointInstruction(
  signer: PublicKey,
  manager: PublicKey,
  roundId: bigint,
  authId: bigint
): TransactionInstruction;

export declare function recycleSolInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint
): TransactionInstruction;

export declare function mmFullAutodeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  checkpointRoundId: bigint,
  amount: bigint,
  squaresMask: number
): TransactionInstruction;

// Miner Creation (manager authority)
export declare function mmCreateMinerInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId?: bigint
): TransactionInstruction;

// Withdraw Tokens (manager authority)
export declare function withdrawTokensInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId?: bigint,
  mint?: PublicKey
): TransactionInstruction;

// Strategy Deployer (manager authority creates, both can update)
export declare function createStratDeployerInstruction(
  signer: PublicKey,
  manager: PublicKey,
  deployAuthority: PublicKey,
  bpsFee: bigint,
  flatFee?: bigint,
  maxPerRound?: bigint,
  strategyType?: number,
  strategyData?: Buffer
): TransactionInstruction;

export declare function updateStratDeployerInstruction(
  signer: PublicKey,
  manager: PublicKey,
  newDeployAuthority: PublicKey,
  newBpsFee: bigint,
  newFlatFee?: bigint,
  newExpectedBpsFee?: bigint,
  newExpectedFlatFee?: bigint,
  newMaxPerRound?: bigint,
  strategyType?: number,
  strategyData?: Buffer
): TransactionInstruction;

// Strategy Autodeploy (deploy authority - for executors)
export declare function mmStratAutodeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  amount: bigint,
  squaresMask: number,
  extra?: number
): TransactionInstruction;

export declare function mmStratFullAutodeployInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint,
  roundId: bigint,
  checkpointRoundId: bigint,
  amount: bigint,
  squaresMask: number,
  extra?: number
): TransactionInstruction;

export declare function mmStratAutocheckpointInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint
): TransactionInstruction;

export declare function recycleStratSolInstruction(
  signer: PublicKey,
  manager: PublicKey,
  authId: bigint
): TransactionInstruction;

// Helpers
export declare function squaresToMask(squares: boolean[]): number;
export declare function maskToSquares(mask: number): boolean[];
