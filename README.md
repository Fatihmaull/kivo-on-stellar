# 💎 Kivo — Smart Account Wallet on Stellar Soroban

> Non-custodial Stellar wallets secured by real device passkeys (secp256r1/WebAuthn) instead of seed phrases, with on-chain social recovery, spending policies, and sponsored (gasless) execution — all enforced by Soroban smart contracts, not client-side trust.

This repository was built for the **Stellar Journey to Mastery** program and has been through a full internal security audit and remediation pass (see [`SECURITY.md`](./SECURITY.md)). The checklists below map every program requirement directly to the code, test, or transaction that satisfies it.

---

## 📌 Level 2 Requirements

| Requirement | Status | Evidence |
|---|---|---|
| StellarWalletsKit implementation | ✅ | [`web/src/lib/wallet.ts`](./web/src/lib/wallet.ts) — real `authModal()` connect flow (Freighter / xBull / Albedo) |
| 3+ error types handled | ✅ | Wallet not found, connection rejected, biometric cancelled/timeout, insufficient testnet balance, on-chain policy/auth errors decoded to readable names — see [Error handling](#️-error-handling) |
| Contract deployed on testnet | ✅ | 4 contracts live — see [Deployed contracts](#-deployed-contracts-testnet) |
| Contract called from the frontend | ✅ | Real Soroban RPC `simulateTransaction`/`sendTransaction` calls in [`web/src/lib/kivo.ts`](./web/src/lib/kivo.ts), not mocked |
| Reading and writing data to a contract | ✅ | Reads: `get_config`, `get_nonce`, `is_guardian`, balances. Writes: deploy, `add_guardian`, `transfer` (passkey- and classic-wallet-signed) |
| Event listening & state synchronization | ✅ | `getEvents` polling hook ([`useWalletEvents`](./web/src/app/page.tsx)) drives the live Activity tab and reconstructs the guardian list — there is no "list guardians" contract method by design (Soroban storage maps aren't enumerable), so this is genuinely load-bearing, not decorative |
| Transaction status tracking (pending/success/fail) | ✅ | [`waitForTransaction`](./web/src/lib/soroban.ts) polls `getTransaction`; every submit flow renders a live status panel with an explorer link |
| Minimum 10+ meaningful commits | ✅ | 29 commits building the original contracts/frontend, plus this session's audit-and-fix pass — see `git log` |

## 📌 Level 3 Requirements

| Requirement | Status | Evidence |
|---|---|---|
| Inter-contract communication | ✅ | `SmartAccount.execute_sponsored` → `Paymaster.is_accepted_token`; `SmartAccount.enforce_policies` → `PolicyEngine.check_policy`; `RecoveryModule` ↔ `SmartAccount` (`is_guardian`, `get_config`, `rotate_credentials`) — all exercised in [`integration_test.rs`](./contracts/smart-account/src/integration_test.rs) and [`recovery-module/src/test.rs`](./contracts/recovery-module/src/test.rs) against real deployed contract instances, not stubs |
| Event streaming & real-time updates | ✅ | Same `getEvents` polling as Level 2, on a 6s interval matching ledger close time |
| CI/CD pipeline setup | ✅ | [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) — contract tests, clippy, a real WASM build for all 4 contracts, frontend lint + typecheck + build, on every push/PR |
| Smart contract deployment workflow | ✅ | Reproducible CLI commands in [Deployment](#-deployment) — this is exactly how the live testnet addresses below were produced |
| Mobile responsive frontend | ✅ | Tailwind responsive layout, verified at 375px viewport with zero horizontal overflow |
| Error handling & loading states | ✅ | Every submit flow has explicit `building → simulating → signing → submitting → pending → success/failed` states, not a single boolean spinner |
| Writing tests for contracts and frontend | ✅ | **46 passing Rust tests** across all 4 contracts (`cargo test --workspace`), including real P-256/ed25519 cryptographic signing — see [Testing](#-testing) |
| Production-ready architecture practices | ✅ | See [`SECURITY.md`](./SECURITY.md) for the full audit: a critical auth bypass found and fixed, replay/storage/recovery hardening, and the reasoning behind each |
| Documentation & demo presentation | ✅ | This README + `SECURITY.md`; demo video excluded per submission scope |

---

## ⚠️ Read this before judging the UI

Two things are true at once, on purpose:

1. **Every contract call in this frontend is real.** No mock data, no `setTimeout`-simulated transactions, no hardcoded balances. Every number on screen came from a Soroban RPC response.
2. **The passkey-signed write path (creating a wallet, sending funds, adding a guardian) has not been exercised against physical biometric hardware by anyone building this** — the automated environment used to build it has no platform authenticator. The WebAuthn↔Soroban signing code in [`web/src/lib/passkey.ts`](./web/src/lib/passkey.ts) and [`web/src/lib/walletSignature.ts`](./web/src/lib/walletSignature.ts) is written to match the contract's verifier byte-for-byte and is covered by **real cryptographic unit tests** (genuine P-256 signatures, genuine DER decoding, genuine low-S normalization) in [`contracts/smart-account/src/test.rs`](./contracts/smart-account/src/test.rs) — but that's sandbox verification of the logic, not a substitute for someone with a real device clicking the real button. **Test this yourself with a device that has TouchID/FaceID/Windows Hello before treating the write path as field-verified.**

Reporting this distinction plainly seemed more useful than a screenshot that can't actually prove a biometric ceremony happened.

---

## 🔗 Deployed Contracts (Testnet)

All four contracts were deployed fresh from this repository's audited code — not the original, vulnerable version. Every address below is independently verifiable on Stellar Expert.

| Contract | Address | Explorer |
|---|---|---|
| **SmartAccount** | `CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY` | [View](https://stellar.expert/explorer/testnet/contract/CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY) |
| **RecoveryModule** | `CAQDZG6KJU5B43NJGA5SCLING2TLYHR7WFDSSD23CHCV2MUPSP6VCMK5` | [View](https://stellar.expert/explorer/testnet/contract/CAQDZG6KJU5B43NJGA5SCLING2TLYHR7WFDSSD23CHCV2MUPSP6VCMK5) |
| **PolicyEngine** | `CAYEAN52QALZNIHAM67I7HP4OJJ42E5DTD7LDA6IJUG7OHNRSNDP2EJS` | [View](https://stellar.expert/explorer/testnet/contract/CAYEAN52QALZNIHAM67I7HP4OJJ42E5DTD7LDA6IJUG7OHNRSNDP2EJS) |
| **Paymaster** | `CD2IV2K2QYWT3TSU6TGSVQOS3EDWN5FUY65K4U2YRK5HQNNU32DVEUBV` | [View](https://stellar.expert/explorer/testnet/contract/CD2IV2K2QYWT3TSU6TGSVQOS3EDWN5FUY65K4U2YRK5HQNNU32DVEUBV) |

### Verifiable transactions

| Call | Tx Hash | Explorer |
|---|---|---|
| `Paymaster.add_accepted_token(nativeXlmSac)` — real state-changing write, admin-authorized | `b25d6c310a5de1397609a1174acfe3709b02d6b29ebdc2f7988c742afd540a53` | [View](https://stellar.expert/explorer/testnet/tx/b25d6c310a5de1397609a1174acfe3709b02d6b29ebdc2f7988c742afd540a53) |
| `SmartAccount.get_config()` — submitted (not just simulated) contract call | `7a4129ca7fb990a0f8b4bf0623c6e2451671e25b08fcb311032adfbe91f20a23` | [View](https://stellar.expert/explorer/testnet/tx/7a4129ca7fb990a0f8b4bf0623c6e2451671e25b08fcb311032adfbe91f20a23) |
| `SmartAccount` deployment (via `__constructor`, atomic — see [`SECURITY.md#k-03`](./SECURITY.md)) | `29c4c459d7543f1244e35c23c608b628c4de0060f03f72d74f8bc01703285b78` | [View](https://stellar.expert/explorer/testnet/tx/29c4c459d7543f1244e35c23c608b628c4de0060f03f72d74f8bc01703285b78) |

The `SmartAccount` instance above was deployed with a deterministic **demo** P-256 owner keypair (not a real device passkey — see the note above) purely so the constructor had valid arguments to verify against. Every wallet a real user creates through the running frontend gets its own contract instance with its own real passkey, deployed live via [`deploySmartAccount`](./web/src/lib/kivo.ts).

---

## 🚀 Key Features

- **🔑 Passkey authentication** — secp256r1/WebAuthn signature verification runs entirely on-chain in `__check_auth`; the challenge binding, RP-ID pin, and User Presence/Verification flags are all enforced by the contract, not trusted from the client (see [`SECURITY.md`](./SECURITY.md) for why this matters).
- **🛡️ Social recovery** — configurable *m*-of-*n* guardian quorum with a timelock, implemented as a genuine cross-contract flow between `SmartAccount` and `RecoveryModule`.
- **⏱️ Spending policy** — a built-in daily limit plus an optional `PolicyEngine` contract for whitelisting and per-token overrides, composed via real inter-contract calls.
- **⛽ Sponsored execution** — `execute_sponsored` runs the user's action and the relayer's fee payment as one atomic call under one signature, bounded by a user-signed `max_fee` the contract itself enforces.
- **⚡ Multi-wallet** — Freighter / xBull / Albedo via StellarWalletsKit for the classic account that pays deployment/network fees; the smart account itself only ever needs a passkey.

---

## 🏗️ Architecture

```
                    ┌─────────────────────────┐
                    │   Classic Wallet         │  pays network fees
                    │  (Freighter/xBull/Albedo)│  (StellarWalletsKit)
                    └────────────┬─────────────┘
                                 │ signs tx envelope
                                 ▼
┌────────────────────────────────────────────────────────────────┐
│                    SmartAccount Contract                       │
│  __check_auth(payload, WalletSignature, contexts)               │
│    ├─ Passkey (secp256r1): challenge-bound WebAuthn verify      │
│    ├─ Ed25519: backup keypair signer                            │
│    └─ SessionKey: scoped, fail-closed, temporary                │
│  ─────────────────────────────────────────────────────────────  │
│  policy::enforce_policies ──────► PolicyEngine.check_policy     │
│  execute_sponsored        ──────► Paymaster.is_accepted_token   │
│  rotate_credentials       ◄────── RecoveryModule (guardian m-of-n)│
└────────────────────────────────────────────────────────────────┘
```

Full mechanism-level detail — including the exact WebAuthn signing pipeline, storage tiering rationale, and the atomic-sponsorship design — is in [`SECURITY.md`](./SECURITY.md).

---

## 💻 Tech Stack

- **Contracts:** Rust, Soroban SDK 22, workspace of 4 contracts + a shared `novus-types` crate
- **Frontend:** Next.js 16 (App Router), TypeScript, Tailwind CSS v4
- **Chain access:** `@stellar/stellar-sdk` (RPC, transaction building, `authorizeEntry` custom-account signing), `@creit.tech/stellar-wallets-kit`
- **Testing:** `cargo test` with real `p256`/`ed25519-dalek` signing (dev-only), GitHub Actions CI
- **Deployment:** Stellar CLI → testnet; Vercel-ready frontend (not deployed as part of this submission — see [Optional: deploying the frontend](#optional-deploying-the-frontend))

---

## ⚙️ Local Development Setup

### Prerequisites
- Rust (stable) + `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli) ≥ 22
- Node.js ≥ 18 + npm
- A WebAuthn-capable browser + device (TouchID/FaceID/Windows Hello) to exercise the passkey flow live

### 1. Clone & install
```bash
git clone https://github.com/Fatihmaull/kivo-on-stellar.git
cd kivo-on-stellar/web
npm install
cd ..
```

### 2. Build & test the contracts
```bash
cargo check --workspace
cargo test --workspace       # 46 tests, see below
cargo clippy --workspace -- -D warnings
```

### 3. Configure the frontend
```bash
cd web
cp .env.example .env.local
```
Fill in your own deployed contract addresses (see [Deployment](#-deployment) below), or reuse the testnet addresses already in this README to point at the live demo contracts.

### 4. Run the frontend
```bash
npm run dev
```
Open [http://localhost:3000](http://localhost:3000). `NEXT_PUBLIC_RP_ID` must match the domain you're serving from — `localhost` works for local dev without HTTPS.

---

## 🧪 Testing

```bash
$ cargo test --workspace
```

**46 tests passing across all 4 contracts** — up from the original 14, and covering code paths the original suite never touched at all:

| Contract | Tests | What's covered |
|---|---|---|
| `smart-account` | 30 | Real P-256 signature verification (valid + 7 distinct rejection cases — wrong challenge, wrong RP, tampered data, replayed nonce, stale epoch after recovery…), session scope enforcement, guardian threshold floor, cross-contract `execute_sponsored` + `PolicyEngine` wiring against real deployed instances |
| `recovery-module` | 5 | Full *m*-of-*n* lifecycle — propose → approve → timelock → execute — against a real `SmartAccount` deployment, not a stub |
| `policy-engine` | 6 | Whitelist enforcement (on/off), per-token limit overrides |
| `paymaster` | 5 | Accepted-token lifecycle, fee margin config, a real Stellar Asset Contract balance moved through `reclaim_fees` |

The K-01 regression suite specifically — `test_assertion_rejected_for_a_different_transaction`, `test_wrong_rp_id_rejected`, `test_missing_user_presence_flag_rejected`, etc. — is what would have caught the critical auth bypass described in `SECURITY.md` before it shipped.

---

## 🛡️ Error Handling

Handled explicitly, each with its own user-facing message (not a generic "something went wrong"):

1. **Wallet extension missing** — StellarWalletsKit's own connect modal shows an "Install" prompt per-wallet; `classifyWalletError` in [`wallet.ts`](./web/src/lib/wallet.ts) covers the fallback text case.
2. **Connection/signature rejected** — distinguished from other failures and shown as "Connection request was rejected," not a stack trace.
3. **Biometric cancelled or timed out** — `NotAllowedError` from `navigator.credentials` mapped to a specific message in [`passkey.ts`](./web/src/lib/passkey.ts).
4. **Unfunded testnet account** — Horizon 404 detected and offered a one-click Friendbot fund instead of a bare error.
5. **On-chain contract errors** — `describeContractError` in [`soroban.ts`](./web/src/lib/soroban.ts) decodes raw `Error(Contract, #302)`-style codes back into names like `SpendingLimitExceeded` using the same error table as the Rust `WalletError` enum.

---

## 🔄 CI/CD

[`.github/workflows/ci.yml`](./.github/workflows/ci.yml) runs on every push/PR:
- `cargo check` / `cargo test --workspace` (46 tests) / `cargo clippy -- -D warnings`
- `cargo build --release --target wasm32v1-none` for all 4 contracts, plus an explicit artifact-existence check — a genuine WASM build gate, not just a native `cargo check`
- `npm run lint` / `npm run build` for the frontend

---

## 📦 Deployment

Commands used to produce the live addresses above — reproducible for anyone with a funded testnet identity:

```bash
# 1. Build & optimize
cargo build --workspace --release --target wasm32v1-none
stellar contract build --optimize   # or: stellar contract optimize --wasm <path>

# 2. Deploy PolicyEngine
stellar contract deploy --wasm target/wasm32v1-none/release/novus_policy_engine.optimized.wasm \
  --source <YOUR_IDENTITY> --network testnet -- --admin <YOUR_ADDRESS>

# 3. Deploy Paymaster
stellar contract deploy --wasm target/wasm32v1-none/release/novus_paymaster.optimized.wasm \
  --source <YOUR_IDENTITY> --network testnet \
  -- --admin <YOUR_ADDRESS> --relayer <YOUR_ADDRESS> --xlm_token <NATIVE_XLM_SAC>

# 4. Deploy SmartAccount (owner args come from a real passkey registration client-side)
stellar contract deploy --wasm target/wasm32v1-none/release/novus_smart_account.optimized.wasm \
  --source <YOUR_IDENTITY> --network testnet \
  -- --owner_credential_id <32-byte hex> --owner_public_key <65-byte hex> \
     --rp_id_hash <32-byte hex> --recovery_threshold 2 \
     --recovery_timelock_ledgers 34560 --default_daily_limit 10000000000000

# 5. Deploy RecoveryModule, bound to the SmartAccount from step 4
stellar contract deploy --wasm target/wasm32v1-none/release/novus_recovery_module.optimized.wasm \
  --source <YOUR_IDENTITY> --network testnet -- --smart_account <SMART_ACCOUNT_ADDRESS>
```

Because every contract uses a `__constructor` (see [`SECURITY.md#k-03`](./SECURITY.md)), deployment and initialization are one atomic step — there's no window where the contract exists but hasn't claimed an owner yet.

### Mainnet readiness

The contracts and frontend are written to the same standard for mainnet, but **mainnet deployment was intentionally not executed as part of this submission** — it requires a funded mainnet account and is a real financial action that belongs to whoever operates the production instance. Before deploying to mainnet:

- [ ] Get an independent security review beyond this internal audit (see `SECURITY.md`'s own caveats section)
- [ ] Replace the demo/self-sponsored deployment flow with a real backend relayer service so onboarding is genuinely gasless
- [ ] Load-test `execute_sponsored` fee quoting against real DEX liquidity
- [ ] Rehearse the full recovery flow (propose → approve → timelock → execute) against a funded mainnet wallet before relying on it
- [ ] Swap `NEXT_PUBLIC_STELLAR_NETWORK=PUBLIC` and point RPC/Horizon URLs at mainnet endpoints

### Optional: deploying the frontend

Not deployed to a public URL as part of this submission (publishing is an outward-facing action outside this repo's scope). To deploy: `vercel --prod` from `web/` after setting the `NEXT_PUBLIC_*` env vars — including `NEXT_PUBLIC_RP_ID` set to the real production domain, since WebAuthn binds passkeys to it permanently.

---

## 📸 Screenshots

Not included in this revision. The originals in a prior version of this README were taken against a UI that hardcoded a fake $12,450.80 balance and simulated every transaction with `setTimeout` — keeping them would have been actively misleading about what the rebuilt app actually does. Real screenshots need a browser with WebAuthn hardware, which the environment this was built in doesn't have. Before submitting, capture:

1. The wallet-selection modal (`Connect Wallet` → StellarWalletsKit's real modal)
2. The dashboard after a real passkey-created wallet, showing its actual (non-fake) on-chain balance
3. The mobile viewport (375px) — verified overflow-free in this repo, worth a screenshot for the record
4. A `cargo test --workspace` run showing `46 passed`
5. The CI pipeline green on GitHub Actions

---

## 📜 License

MIT License. Built for the Stellar Journey to Mastery program.
