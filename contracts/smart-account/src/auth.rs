/// Core authentication logic for `__check_auth`.
///
/// Pipeline:
/// 1. Nonce verification & replay protection
/// 2. Signer lookup — Persistent (Passkey/Ed25519) first, then Temporary
///    (SessionKey) if no Persistent match exists
/// 3. Cryptographic signature verification (secp256r1 or ed25519)
/// 4. Credential-epoch check (rejects anything predating the last recovery)
/// 5. Session key scope validation (if applicable) — replaces general
///    policy enforcement, since a session has its own tighter caps
/// 6. Policy enforcement (spending limits, PolicyEngine) for Passkey/Ed25519
/// 7. TTL piggyback refresh for critical storage entries
use soroban_sdk::{auth::Context, crypto::Hash, Env, Vec};

use novus_types::{SignerType, WalletError, WalletSignature};

use crate::nonce;
use crate::policy;
use crate::session;
use crate::storage::Storage;
use crate::webauthn;

pub fn check_auth(
    env: &Env,
    signature_payload: &Hash<32>,
    signature: &WalletSignature,
    auth_contexts: &Vec<Context>,
) -> Result<(), WalletError> {
    // ── STEP 1: Replay Protection (Nonce + Payload Hash Guard) ──
    nonce::verify_and_increment_nonce(env, signature, signature_payload)?;

    // ── STEP 2/3: Signer lookup + signature verification ──
    // Persistent signers (Passkey/Ed25519) are tried first; if the
    // credential isn't registered there, it may be a live session key.
    if Storage::has_signer(env, &signature.credential_id) {
        let signer = Storage::get_signer(env, &signature.credential_id)?;
        let config = Storage::get_config(env)?;

        // A signer registered before the wallet's last social recovery is
        // dead on arrival — recovery may have been triggered precisely
        // because this credential was compromised.
        if signer.epoch != config.credential_epoch {
            return Err(WalletError::StaleCredentialEpoch);
        }

        match signer.signer_type {
            SignerType::Passkey => {
                webauthn::verify_passkey(
                    env,
                    &signer.public_key,
                    &config.rp_id_hash,
                    signature_payload,
                    &signature.authenticator_data,
                    &signature.client_data_json,
                    &signature.signature_bytes,
                )?;
            }
            SignerType::Ed25519 => {
                webauthn::verify_ed25519_signature(
                    env,
                    &signer.public_key,
                    signature_payload,
                    &signature.signature_bytes,
                );
            }
        }

        // ── STEP 6: Policy enforcement ──
        policy::enforce_policies(env, auth_contexts)?;

        // ── STEP 7: Piggyback TTL Refresh ──
        Storage::extend_instance_ttl(env);
        Storage::bump_signer_ttl(env, &signature.credential_id);

        return Ok(());
    }

    if Storage::has_session(env, &signature.credential_id) {
        // Signature + scope + spend-cap verification all happen together
        // here — a session key's scope check *is* its policy enforcement.
        session::verify_session(
            env,
            &signature.credential_id,
            signature_payload,
            &signature.signature_bytes,
            auth_contexts,
        )?;

        Storage::extend_instance_ttl(env);
        return Ok(());
    }

    Err(WalletError::SignerNotFound)
}
