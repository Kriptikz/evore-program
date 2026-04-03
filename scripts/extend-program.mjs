#!/usr/bin/env node

// Bypasses the solana CLI's client-side upgrade-authority check for `program extend`.
// The on-chain ExtendProgram instruction does NOT require the upgrade authority —
// anyone can extend a program and pay the additional rent.
//
// Usage:
//   node extend-program.mjs <program-id> <additional-bytes>
//
// Reads RPC URL and keypair path from `solana config get` (Solana CLI config).

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import fs from "fs";
import os from "os";
import path from "path";

const BPF_LOADER_UPGRADEABLE = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111"
);

function readSolanaConfig() {
  const configPath = path.join(
    os.homedir(),
    ".config",
    "solana",
    "cli",
    "config.yml"
  );
  const content = fs.readFileSync(configPath, "utf-8");
  const config = {};
  for (const line of content.split("\n")) {
    const match = line.match(/^(\w[\w_]*)\s*:\s*(.+)$/);
    if (match) {
      config[match[1]] = match[2].trim();
    }
  }
  return config;
}

function loadKeypair(keypairPath) {
  const resolved = keypairPath.replace(/^~/, os.homedir());
  const secretKey = JSON.parse(fs.readFileSync(resolved, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.error(
      "Usage: node extend-program.mjs <program-id> <additional-bytes>"
    );
    process.exit(1);
  }

  const programId = new PublicKey(args[0]);
  const additionalBytes = parseInt(args[1], 10);

  if (isNaN(additionalBytes) || additionalBytes <= 0) {
    console.error("Error: additional-bytes must be a positive integer");
    process.exit(1);
  }

  const config = readSolanaConfig();
  const connection = new Connection(config.json_rpc_url, "confirmed");
  const payer = loadKeypair(config.keypair_path);

  const [programDataAddress] = PublicKey.findProgramAddressSync(
    [programId.toBuffer()],
    BPF_LOADER_UPGRADEABLE
  );

  console.log(`Program ID:       ${programId.toBase58()}`);
  console.log(`ProgramData:      ${programDataAddress.toBase58()}`);
  console.log(`Payer:            ${payer.publicKey.toBase58()}`);
  console.log(`Additional bytes: ${additionalBytes}`);

  const rentCost = await connection.getMinimumBalanceForRentExemption(
    additionalBytes
  );
  console.log(
    `Estimated cost:   ${(rentCost / 1e9).toFixed(6)} SOL`
  );
  console.log();

  // ExtendProgram instruction: u32 LE variant (6) + u32 LE additional_bytes
  const data = Buffer.alloc(8);
  data.writeUInt32LE(6, 0);
  data.writeUInt32LE(additionalBytes, 4);

  const instruction = new TransactionInstruction({
    keys: [
      { pubkey: programDataAddress, isSigner: false, isWritable: true },
      { pubkey: programId, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
    ],
    programId: BPF_LOADER_UPGRADEABLE,
    data,
  });

  const tx = new Transaction().add(instruction);

  console.log("Sending extend transaction...");
  const sig = await sendAndConfirmTransaction(connection, tx, [payer]);
  console.log(`Success! Signature: ${sig}`);
  console.log(`Program extended by ${additionalBytes} bytes.`);
}

main().catch((err) => {
  console.error("Error:", err.message);
  process.exit(1);
});
