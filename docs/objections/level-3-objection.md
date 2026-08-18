# Objection — Level 3 (Orange Belt) Review

| | |
|---|---|
| **Verdict received** | Revisions Needed |
| **Reviewed** | 2026-08-17 17:53:45 |
| **Commit this review evidently graded** | pre-audit snapshot (before `76c6659`) |
| **Current commit** | [`95d0faa`](https://github.com/Fatihmaull/kivo-on-stellar/commit/95d0faa) |
| **Repository** | [github.com/Fatihmaull/kivo-on-stellar](https://github.com/Fatihmaull/kivo-on-stellar) |

## Summary

This is the same repository the Level 2 review assessed, timestamped within 5 seconds of it — both appear to have been generated from the same stale, pre-audit snapshot. Every flagged item describes something that specifically did not exist in the repository before its security-audit-and-rebuild pass, and does exist now. The clearest single piece of evidence: finding #5 states no `@stellar/stellar-sdk` integration file is present in the judged subset. The current repository has seven of them.

## Point by point

### 1. Connect Wallet Feature Check — objection

> "No frontend or wallet integration code appears in the judged subset; README only claims StellarWalletsKit support."

Wallet integration is in [`web/src/lib/wallet.ts`](../../web/src/lib/wallet.ts) — real StellarWalletsKit v2.5 calls (`authModal()`, `signTransaction()`, `disconnect()`) — with a real "Connect Wallet" button in [`web/src/app/page.tsx`](../../web/src/app/page.tsx). Driven live in a real, connected browser session during this repo's development, with the actual wallet-selection modal rendering all three configured wallets.

### 2. Smart Contract Folder Structure Check — passed, no objection

### 3. Smart Contract Code Validation — passed, no objection

### 4. README and Deployment Validation — objection

> "the deployed contract ID/account/tx hash look like placeholder values (repeated 'XYZ7' pattern, unresolved video/screenshot links)"

Neither pattern exists in the current repository:

```bash
grep -n "XYZ" README.md
# (no matches)
grep -n "Link to Demo Video" README.md
# (no matches — the demo video was excluded from this submission by explicit instruction, not left as a dead placeholder)
```

The real deployed address, independently verifiable on Stellar Expert with matching `__constructor` arguments and a live `get_config()` call:

```
CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY
```
https://stellar.expert/explorer/testnet/contract/CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY

### 5. Smart Contract Integration Codebase Check — objection

> "No @stellar/stellar-sdk integration file (soroban.js/ts, contract.js/ts, services, hooks, etc.) is present in the judged subset. No contract initialization, TransactionBuilder, RPC, or sendTransaction code can be verified."

This is the clearest evidence of a stale review. `web/src/lib/` currently contains seven integration files:

```
env.ts  kivo.ts  passkey.ts  soroban.ts  storage.ts  wallet.ts  walletSignature.ts
```

`soroban.ts` and `kivo.ts` both contain real `TransactionBuilder`, `simulateTransaction`, and `sendTransaction` calls against live Soroban RPC (`soroban-testnet.stellar.org`). None of this directory existed in the version this review describes — the pre-audit frontend was a single ~1,100-line `page.tsx` with no `lib/` directory, and every "transaction" in it was implemented as a `setTimeout` simulation rather than a real network call.

### 6. Cross-Check Contract and Frontend Function Matching — objection

> "Without frontend integration code in the judged subset, there is no evidence that the UI calls the contract functions defined in lib.rs"

The frontend calls the real, current contract method names directly:

| Frontend call | Contract definition |
|---|---|
| `page.tsx`: `method: "transfer"` | SAC `transfer`, invoked via the passkey-authorized execute path |
| `page.tsx`: `method: "add_guardian"` | `SmartAccountContract::add_guardian` in [`contracts/smart-account/src/lib.rs`](../../contracts/smart-account/src/lib.rs) |
| `kivo.ts`: `"get_config"` | `SmartAccountContract::get_config` in [`contracts/smart-account/src/lib.rs`](../../contracts/smart-account/src/lib.rs) |

## Request

Please re-run the review against `main` at commit `95d0faa` rather than the snapshot this review was generated from. Findings 1, 4, 5, and 6 are all resolved in the current commit; findings 2 and 3 were already assessed correctly and are unaffected by this objection.
