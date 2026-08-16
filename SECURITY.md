# Security Audit & Remediation

This document records an internal audit of the Kivo smart contracts performed before this submission, the critical finding it surfaced, and every fix that followed. It exists so a reviewer doesn't have to take "production-ready" on faith — the findings, the fix, and the regression test for each are all linked.

**Scope:** the 4 contracts in `contracts/` and the shared `crates/novus-types` crate, as of the pre-audit commit `92f7674`. **Not in scope:** the relayer/paymaster backend (none exists — see the note in the main README), third-party dependencies (`soroban-sdk` itself), and anything outside this repository.

---

## The critical finding: passkey signatures weren't bound to a transaction

**File:** `contracts/smart-account/src/webauthn.rs` (pre-fix) · **Severity:** Critical

The verifier checked `sha256(authenticator_data ‖ client_data_json_hash)` against the stored secp256r1 public key — but `signature_payload`, the actual transaction hash the Soroban host asks `__check_auth` to authorize, was **never read**. And `client_data_json_hash` was a `BytesN<32>` the *caller* supplied, not derived from anything the contract could verify.

The signature proved only "this passkey signed something, at some point." Concretely:

1. Every past authorization is public in a transaction's XDR on Horizon.
2. Read the `(authenticator_data, client_data_json_hash, signature_bytes)` triple off any prior transaction from this wallet.
3. Call `get_nonce()` — a public read.
4. Build a *new* transaction (e.g., draining the wallet's balance), attach the stolen triple with the current nonce.
5. `secp256r1_verify` re-verifies the *old* signed message, which is still validly signed. It passes.

### The fix

`WalletSignature.client_data_json_hash: BytesN<32>` became `client_data_json: Bytes` — the raw bytes, not a caller-asserted hash of them. The verifier ([`webauthn.rs`](./contracts/smart-account/src/webauthn.rs)) now:

1. Base64url-encodes `signature_payload` and asserts it appears at the `"challenge":"` offset inside the raw `clientDataJSON` — this is what binds the signature to *this* transaction.
2. Confirms `"type":"webauthn.get"` — rejects a registration ceremony (`webauthn.create`) replayed as an authorization.
3. Confirms `authenticatorData`'s `rpIdHash` matches the wallet's configured origin (`WalletConfig.rp_id_hash`, set at construction).
4. Confirms the User Present (UP) and User Verified (UV) flag bits are set — a human touched the authenticator and the biometric/PIN gate actually ran.
5. Only then reconstructs `authenticatorData ‖ SHA-256(clientDataJSON)` and calls `secp256r1_verify`.

**Regression coverage:** `test_valid_assertion_authorizes_its_own_payload`, `test_assertion_rejected_for_a_different_transaction` (the exact exploit above, now asserted to fail with `ChallengeMismatch`), `test_wrong_ceremony_type_rejected`, `test_wrong_rp_id_rejected`, `test_missing_user_presence_flag_rejected`, `test_missing_user_verification_flag_rejected`, `test_tampered_authenticator_data_fails_signature_check` — all in [`contracts/smart-account/src/test.rs`](./contracts/smart-account/src/test.rs), all signing with a real P-256 keypair (`p256` crate, dev-only), not a stub.

---

## Full findings

| ID | Severity | Finding | Fix | Regression test |
|---|---|---|---|---|
| K-01 | **Critical** | Passkey challenge never bound to the transaction (above) | Raw `clientDataJSON` + base64url challenge assertion + RP-ID pin + UP/UV flags | `test_assertion_rejected_for_a_different_transaction` + 5 others |
| K-02 | High | `session.rs`: `if !allowed.is_empty() && !contains` — an **empty whitelist permitted everything**, including a session key calling the wallet's own `add_signer` | Fail-closed: empty list denies. Session creation now refuses an empty `allowed_contracts`/`allowed_functions`, and the wallet's own address is hard-denied as a session target regardless of whitelist | `test_session_creation_rejects_empty_whitelist`, `test_session_creation_rejects_self_as_target`, `test_session_key_cannot_target_the_wallet_itself` |
| K-03 | High | `initialize()` was a separate, unauthenticated call — a window existed between "contract deployed" and "contract has an owner" for anyone to claim it | All 4 contracts moved to `__constructor`, invoked atomically with deployment | Structural — the vulnerable window no longer exists to test against |
| K-04 | High | Recovery timelock expiry lived in Temporary storage; if archived, `execute_recovery` could never succeed again — a permanently stuck proposal | Expiry moved onto the Persistent `RecoveryProposal` itself | `test_full_recovery_lifecycle` (advances the ledger well past the timelock and confirms execution still succeeds) |
| K-05 | High | `Paymaster.collect_fee` let the relayer pull an unbounded, relayer-chosen amount | Replaced with `SmartAccount.execute_sponsored`: action + fee transfer in one atomic call, bounded by a caller-signed `max_fee` the contract itself enforces before moving anything | `test_execute_sponsored_rejects_fee_over_max`, `test_execute_sponsored_moves_action_and_fee_atomically` |
| K-06 | Medium | Daily spend accounting counted *inbound* transfers (receiving funds) against the *outbound* daily limit | Only meters when `args[0] == self` (an actual send) | Covered by `test_execute_sponsored_moves_action_and_fee_atomically`'s balance assertions |
| K-07 | Medium | A session key was both a Persistent `SignerEntry` and a Temporary `SessionConfig`; once the Temporary half expired, the Persistent half remained forever, billed and useless | Consolidated into one self-contained Temporary `SessionConfig` (now carries `public_key` directly) — nothing about a session outlives its own TTL | Structural; `test_session_key_executes_within_scope` exercises the consolidated record end-to-end |
| K-08 | Medium | `rotate_owner` (the final step of recovery) swapped the owner key but left every other signer/session untouched — anything an attacker added before triggering recovery survived it | Added `credential_epoch` to `WalletConfig` and every `SignerEntry`/`SessionConfig`; rotation bumps it, and `__check_auth` rejects any credential stamped with a stale epoch — no enumeration needed | `test_rotated_owner_credential_is_invalidated` |
| K-09 | Medium | A hand-rolled sequential nonce + payload-hash replay guard duplicated what the Soroban host's own auth-entry nonce already does — at a cost of extra storage writes per transaction, and a griefing surface (burn the nonce, invalidate someone's signed tx) | *Kept, deliberately* — see [Design decisions](#design-decisions) below | — |
| K-10 | Medium | `remove_guardian` had no floor check — removing guardians could drop the count below `recovery_threshold`, making recovery permanently unreachable | Refuses removal if `guardian_count - 1 < recovery_threshold` | `test_remove_guardian_below_threshold_fails`, `test_remove_guardian_above_threshold_succeeds` |
| K-11 | Medium | `PolicyEngine.check_policy`'s whitelist branch was two empty comment blocks — a structural no-op that always returned `Ok` | Implemented for real, gated behind an explicit `WhitelistEnforced` flag so adopting the engine can't retroactively lock out an existing wallet | `test_whitelist_enforced_rejects_unlisted_contract`, `test_whitelist_disabled_by_default` |
| — | Design | `policy-engine` and `paymaster` were deployed contracts with **no caller anywhere in the codebase** — dead weight | Wired into `smart-account` via real cross-contract calls (`policy::enforce_policies` → `PolicyEngine.check_policy`; `execute_sponsored` → `Paymaster.is_accepted_token`) | `test_policy_engine_whitelist_blocks_unlisted_target`, all `execute_sponsored` tests |

---

## Design decisions worth explaining

**Why the custom nonce (K-09) stayed.** The Soroban host's own auth-entry nonce is cryptographically bound into the signed preimage and already prevents replay — the hand-rolled counter here is redundant, not broken. Removing it would mean rebuilding the client-side signing flow (in `web/src/lib/kivo.ts`) around the protocol nonce instead, which is real work with its own risk of introducing a new bug under time pressure. Given the counter is *inefficient*, not *exploitable*, keeping it was the more conservative call for this pass. It's the clearest candidate for a follow-up cleanup.

**Why `SignerType::Guardian` and `SignerType::SessionKey`-as-a-Persistent-signer were removed, not just fixed.** Both were representable states that never needed to exist — a guardian is never looked up by credential ID (it's addressed by `Address` through a completely separate code path), and a session key's identity now lives entirely in its own Temporary record. Removing the states outright, rather than patching around them, means there's no dead branch left for a future change to silently reawaken.

**Why `execute_sponsored` lives on `SmartAccount`, not `Paymaster`.** Soroban authorization is scoped to a call tree declared up front by the client and matched against what actually executes. Putting the entry point on the wallet means the target action and the fee transfer are both sub-invocations of one call the wallet signs once — they succeed or fail together by construction, not by convention.

---

## What this audit does not cover

- **No external review.** This is one internal pass, not a third-party audit. Treat it as a floor, not a ceiling, before mainnet.
- **The relayer backend does not exist.** `execute_sponsored`'s contract-side logic is real and tested; a production gasless-onboarding flow needs a server that actually runs as the relayer. This repo's frontend self-sponsors via a connected classic wallet instead — see the main README.
- **The passkey signing path is unit-tested, not device-tested.** Real cryptography, real byte layouts, verified against the actual contract logic — but no one has clicked "Create Account with Passkey" on a real device as part of building this. See the note in the main README.
