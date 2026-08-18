# Objection — Level 2 (Yellow Belt) Review

| | |
|---|---|
| **Verdict received** | Revisions Needed |
| **Reviewed** | 2026-08-17 17:53:50 |
| **Commit this review evidently graded** | pre-audit snapshot (before `76c6659`) |
| **Current commit** | [`95d0faa`](https://github.com/Fatihmaull/kivo-on-stellar/commit/95d0faa) |
| **Repository** | [github.com/Fatihmaull/kivo-on-stellar](https://github.com/Fatihmaull/kivo-on-stellar) |

## Summary

Every item this review flags traces to text that existed in an earlier version of the repository and has since been replaced. The concrete tell is the "XYZ" placeholder pattern quoted in finding #3 — that string does not exist anywhere in the current repository (`grep -n "XYZ" README.md` returns zero matches) but is an exact match for a `.env` example value from before this repo's security-audit-and-rebuild pass. This submission appears to have been locked to a commit at the time it was first queued, before that rebuild landed.

## Point by point

### 1. Smart Contract Folder Structure Check — passed, no objection

### 2. Smart Contract Code Validation — passed, no objection

### 3. README and Deployment Validation — objection

> "the claimed deployed contract address and transaction hash appear to be placeholders (repeated XYZ pattern / fabricated-looking hash)"

The pattern described (`CA3WNWV3W4PKE5B2Z47XYZ7XYZ7XYZ7XYZ7XYZ7XYZ7XYZ7XYZ7XYZ`) was a placeholder in an old `.env` example block. It is not present anywhere in the current repository:

```bash
grep -n "XYZ" README.md web/.env.example
# (no matches)
```

The current README lists a real deployed `SmartAccount` contract:

```
CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY
```

Independently verifiable on Stellar Expert right now — the page shows the real creation transaction, the exact `__constructor` arguments, and a subsequent `get_config()` call returning matching on-chain state:

https://stellar.expert/explorer/testnet/contract/CDXSGRILJX2OY3R4GF2J6SBWWJXVV5NYRI7NVMESH47UXSKHIEAKJ5DY

A real, **submitted** (not just simulated) contract-call transaction is also listed and independently verifiable:

```
7a4129ca7fb990a0f8b4bf0623c6e2451671e25b08fcb311032adfbe91f20a23
```
https://stellar.expert/explorer/testnet/tx/7a4129ca7fb990a0f8b4bf0623c6e2451671e25b08fcb311032adfbe91f20a23

### 4. Connect Wallet Feature Check — objection

> "no web source in the inspected files could be checked for getAddress, setAllowed, signTransaction, or a visible connect button"

`getAddress` and `setAllowed` are method names from a pre-2.x version of `@creit.tech/stellar-wallets-kit`. The version installed in this repo is `2.5.0`, whose API does not include those methods at all — code calling them would fail to compile against this dependency.

What the current SDK version actually exposes, and what this repo uses, is directly present in [`web/src/lib/wallet.ts`](../../web/src/lib/wallet.ts):

```ts
await kit.StellarWalletsKit.authModal();               // opens the real wallet-selection modal
await kit.StellarWalletsKit.signTransaction(xdr, ...);  // literal signTransaction call
```

A visible connect button is literally present in [`web/src/app/page.tsx`](../../web/src/app/page.tsx) (search `Connect Wallet`), and was driven live in a real, connected browser session during this repo's development — the real StellarWalletsKit modal rendered with all three configured wallets (Freighter, xBull, Albedo).

## Request

Please re-run the review against `main` at commit `95d0faa` rather than the snapshot this review was generated from. All flagged items are resolved in the current commit.
