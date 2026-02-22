# Evore Program Deployment Runbook

Verified build and Squads multisig upgrade flow for the Evore on-chain program.

> **This runbook is designed to be executed step-by-step with an AI assistant (e.g. Claude).** Each step should be run sequentially, confirming output before proceeding to the next. Do not skip ahead.

## Prerequisites

| Tool | Install |
|------|---------|
| `solana` CLI | [docs.solanalabs.com](https://docs.solanalabs.com/cli/install) |
| `solana-verify` | `cargo install solana-verify` |
| `vbi` | `cargo +1.88.0 install verify-buffer-integrity` (requires Rust >= 1.88) |
| Docker | [docker.com](https://www.docker.com/products/docker-desktop/) — must be running for Step 1 (build) |

Confirm everything is ready:

```bash
solana --version
solana-verify --version
vbi --help
docker info
```

## Configuration

```bash
PROGRAM_ID="8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa"
MULTISIG_AUTHORITY="Cy1uYhDqrpi6RBV6GcX28EhGfk31kM7ccpWCptjjzmGc"
GITHUB_REPO="https://github.com/kriptikz/evore-program"
LIBRARY_NAME="evore"
```

| Variable | Description |
|----------|-------------|
| `PROGRAM_ID` | On-chain program address |
| `MULTISIG_AUTHORITY` | Squads multisig that owns the program upgrade authority |
| `GITHUB_REPO` | GitHub repository URL (no `.git` suffix) |
| `LIBRARY_NAME` | Cargo lib name — must match `[lib] name` in `program/Cargo.toml` |

Verify Solana CLI is pointed at mainnet and using the correct deployer keypair:

```bash
solana config get
```

---

## Step 0: Review changes to be deployed

Check what commit is currently verified on-chain and compare with local HEAD.

```bash
curl -s "https://verify.osec.io/status/${PROGRAM_ID}" | python3 -m json.tool
```

If a prior verified commit exists, diff against it:

```bash
DEPLOYED_COMMIT="<commit from verify API>"
git log --oneline $DEPLOYED_COMMIT..HEAD
git diff --stat $DEPLOYED_COMMIT..HEAD
```

If the program has never been verified, just confirm your working tree is clean and HEAD is what you want to deploy:

```bash
git status
git log --oneline -5
```

> **Do not proceed until you are confident HEAD is the correct code to ship.**

---

## Step 1: Build with Docker (reproducible / verified)

```bash
solana-verify build --library-name evore
```

This runs `cargo build-sbf` inside the Solana Foundation's deterministic Docker image. Output artifact: `target/deploy/evore.so`.

First run pulls the Docker image and installs toolchains (~10-15 min). Subsequent builds use cache and are faster.

Confirm the artifact and its hash:

```bash
ls -lh target/deploy/evore.so
solana-verify get-executable-hash target/deploy/evore.so
```

Record the executable hash — you'll compare it after verification.

---

## Step 2: Create buffer keypair

```bash
BUFFER_KEYPAIR="/tmp/${LIBRARY_NAME}-buffer-$(date +%s).json"
solana-keygen new -o "$BUFFER_KEYPAIR" --no-bip39-passphrase --force
BUFFER_ADDRESS=$(solana address -k "$BUFFER_KEYPAIR")
echo "BUFFER_KEYPAIR=$BUFFER_KEYPAIR"
echo "BUFFER_ADDRESS=$BUFFER_ADDRESS"
```

Save both values — they're used in later steps.

---

## Step 3: Write program to buffer

```bash
solana program write-buffer target/deploy/evore.so \
  --buffer "$BUFFER_KEYPAIR" \
  --with-compute-unit-price 0 \
  -um
```

> **Note:** Start with `--with-compute-unit-price 0`. If the transaction doesn't land, bump to `10000` or `50000`. Avoid excessively high values.

The deployer wallet pays rent for the buffer (~1.5 SOL for this program). This is refunded when the buffer is consumed during upgrade.

---

## Step 4: Verify buffer integrity

```bash
vbi --program-file target/deploy/evore.so --buffer-address "$BUFFER_ADDRESS"
```

Expected output — hashes must match:

```
file hash   = <hash>
buffer hash = <hash>
```

If they don't match, do **not** proceed. Re-check your build and buffer write.

---

## Step 5: Export verification PDA transaction

This generates a base58-encoded transaction that writes verification metadata (repo URL, commit hash) to the Solana Verify on-chain PDA. You'll submit this through Squads.

> **Required after every upgrade.** The PDA records which commit produced the on-chain binary. New commit = new PDA tx.

```bash
solana-verify export-pda-tx \
  "$GITHUB_REPO" \
  --library-name "$LIBRARY_NAME" \
  --program-id "$PROGRAM_ID" \
  --uploader "$MULTISIG_AUTHORITY" \
  --encoding base58 \
  --compute-unit-price 0
```

Copy the full base58 output — you'll paste it in Squads as a raw/serialized transaction.

---

## Step 6: Create proposals in Squads

You need **two proposals** in Squads:

### 6a. Program upgrade proposal

1. Open [Squads](https://v4.squads.so/) and navigate to your multisig.
2. Create a **Program Upgrade** transaction:
   - **Program ID:** `8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa`
   - **Buffer:** the `BUFFER_ADDRESS` from Step 2
   - **Spill / Refund:** your deployer wallet address (the one that funded the buffer)
3. Get the commit link for the proposal description:
   ```bash
   echo "https://github.com/kriptikz/evore-program/commit/$(git rev-parse HEAD)"
   ```

### 6b. Verification PDA proposal

1. In Squads, use the **import raw/serialized transaction** option.
2. Paste the base58 payload from Step 5.
3. This writes verification metadata on-chain under the multisig uploader.

---

## Step 7: Transfer buffer authority to multisig

Run this **after** creating the upgrade proposal in Squads but **before** executing it:

```bash
solana program set-buffer-authority "$BUFFER_ADDRESS" \
  --new-buffer-authority "$MULTISIG_AUTHORITY" \
  -um
```

Confirm:

```
Account Type: Buffer
Authority: Cy1uYhDqrpi6RBV6GcX28EhGfk31kM7ccpWCptjjzmGc
```

---

## Step 8: Approve and execute in Squads

1. Collect required multisig approvals for both proposals.
2. Execute the **program upgrade** proposal.
3. Execute the **verification PDA** proposal.

> **Do not proceed to Step 9 until both transactions are confirmed on-chain.**

---

## Step 9: Submit remote verification job

```bash
solana-verify remote submit-job \
  --program-id "$PROGRAM_ID" \
  --uploader "$MULTISIG_AUTHORITY"
```

This tells Solana Verify to rebuild from source and compare against the on-chain binary. May take a minute. Retry if rate-limited.

Expected output:

```
Program 8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa has been verified. ✅
```

Check status anytime at:

```
https://verify.osec.io/status/8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa
```

---

## Step 10: Cleanup

Remove the temporary buffer keypair:

```bash
rm -f "$BUFFER_KEYPAIR"
```

---

## Quick Reference

| Item | Value |
|------|-------|
| Program ID | `8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa` |
| Multisig Authority | `Cy1uYhDqrpi6RBV6GcX28EhGfk31kM7ccpWCptjjzmGc` |
| Repo | `https://github.com/kriptikz/evore-program` |
| Library Name | `evore` |
| Verify Status | `https://verify.osec.io/status/8jaLKWLJAj5jVCZbxpe3zRUvLB3LD48MRtaQ2AjfCfxa` |
| Rust Toolchain (repo) | `1.85.0` (pinned in `rust-toolchain.toml`) |
| Docker Image | `solanafoundation/solana-verifiable-build` (auto-selected by `solana-verify`) |
