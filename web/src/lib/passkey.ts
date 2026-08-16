import { xdr } from "@stellar/stellar-sdk";
import { buildWalletSignatureScVal, bytesToHex, derSignatureToRaw, sha256 } from "./walletSignature";

export class PasskeyError extends Error {
  constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "PasskeyError";
  }
}

export function isWebAuthnSupported(): boolean {
  return typeof window !== "undefined" && !!window.PublicKeyCredential && !!navigator.credentials;
}

export interface CreatedPasskey {
  /** The authenticator's own credential ID — pass this back into
   * `allowCredentials` on every subsequent `get()` call. */
  rawCredentialId: Uint8Array;
  /** SHA-256(rawCredentialId) — the 32-byte value actually stored on-chain
   * as `SignerEntry.credential_id`, since real authenticator credential IDs
   * are variable-length and the contract's field is a fixed `BytesN<32>`. */
  credentialId32: Uint8Array;
  /** Raw uncompressed secp256r1 point (0x04 ‖ X ‖ Y), 65 bytes — exactly
   * what `SignerEntry.public_key` stores. */
  publicKeyPoint65: Uint8Array;
}

/** Register a new passkey via a real WebAuthn ceremony (biometric/PIN
 * prompt). Throws `PasskeyError` with a message suitable for direct
 * display on cancellation, timeout, or an unsupported platform. */
export async function createPasskey(rpId: string, rpName: string, accountLabel: string): Promise<CreatedPasskey> {
  if (!isWebAuthnSupported()) {
    throw new PasskeyError("This browser doesn't support passkeys (WebAuthn). Try Chrome, Safari, or Edge on a device with biometrics or a PIN.");
  }

  const challenge = crypto.getRandomValues(new Uint8Array(32));
  const userId = crypto.getRandomValues(new Uint8Array(16));

  let credential: PublicKeyCredential;
  try {
    credential = (await navigator.credentials.create({
      publicKey: {
        challenge,
        rp: { name: rpName, id: rpId },
        user: { id: userId, name: accountLabel, displayName: accountLabel },
        pubKeyCredParams: [{ type: "public-key", alg: -7 }], // ES256 / secp256r1
        timeout: 60_000,
        authenticatorSelection: { authenticatorAttachment: "platform", userVerification: "required", residentKey: "preferred" },
        attestation: "none",
      },
    })) as PublicKeyCredential;
  } catch (err) {
    throw classifyWebAuthnError(err);
  }

  if (!credential) throw new PasskeyError("Passkey creation returned no credential.");

  const response = credential.response as AuthenticatorAttestationResponse;
  const rawCredentialId = new Uint8Array(credential.rawId);

  let publicKeyPoint65: Uint8Array;
  if (typeof response.getPublicKey === "function") {
    const spki = response.getPublicKey();
    if (!spki) throw new PasskeyError("Authenticator did not return a public key.");
    // SEC1 uncompressed point (0x04 ‖ X ‖ Y) is the trailing 65 bytes of the
    // SubjectPublicKeyInfo DER structure for a P-256 key.
    const spkiBytes = new Uint8Array(spki);
    publicKeyPoint65 = spkiBytes.slice(spkiBytes.length - 65);
    if (publicKeyPoint65[0] !== 0x04) {
      throw new PasskeyError("Authenticator public key was not an uncompressed P-256 point — this device may not support the required algorithm.");
    }
  } else {
    throw new PasskeyError("This browser's WebAuthn implementation doesn't expose getPublicKey(); try a recent Chrome, Safari, or Edge.");
  }

  const credentialId32 = await sha256(rawCredentialId);

  return { rawCredentialId, credentialId32, publicKeyPoint65 };
}

/** A single WebAuthn assertion, already decoded into what the contract
 * verifier (`webauthn.rs`) expects: raw authenticatorData, raw
 * clientDataJSON, and a low-S-normalized 64-byte (r ‖ s) signature. */
export interface Assertion {
  authenticatorData: Uint8Array;
  clientDataJSON: Uint8Array;
  signatureRaw64: Uint8Array;
}

/** Request a WebAuthn assertion authorizing `challenge` (the exact 32-byte
 * `signature_payload` the contract will check the assertion against). */
export async function getAssertion(rawCredentialId: Uint8Array, challenge: Uint8Array): Promise<Assertion> {
  if (!isWebAuthnSupported()) {
    throw new PasskeyError("This browser doesn't support passkeys (WebAuthn).");
  }

  let credential: PublicKeyCredential;
  try {
    credential = (await navigator.credentials.get({
      publicKey: {
        challenge: challenge as BufferSource,
        allowCredentials: [{ type: "public-key", id: rawCredentialId as BufferSource }],
        userVerification: "required",
        timeout: 60_000,
      },
    })) as PublicKeyCredential;
  } catch (err) {
    throw classifyWebAuthnError(err);
  }

  if (!credential) throw new PasskeyError("Biometric verification returned no credential.");

  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    authenticatorData: new Uint8Array(response.authenticatorData),
    clientDataJSON: new Uint8Array(response.clientDataJSON),
    signatureRaw64: derSignatureToRaw(new Uint8Array(response.signature)),
  };
}

/** Build a full `WalletSignature` ScVal for a Passkey-type signer, ready to
 * drop straight into a `SorobanAuthorizationEntry`'s `signature` field. */
export async function signPasskeyAuth(
  rawCredentialId: Uint8Array,
  credentialId32: Uint8Array,
  nonce: bigint,
  challenge32: Uint8Array
): Promise<xdr.ScVal> {
  const assertion = await getAssertion(rawCredentialId, challenge32);
  return buildWalletSignatureScVal({
    credentialId: credentialId32,
    signatureBytes: assertion.signatureRaw64,
    authenticatorData: assertion.authenticatorData,
    clientDataJson: assertion.clientDataJSON,
    nonce,
  });
}

function classifyWebAuthnError(err: unknown): PasskeyError {
  const e = err as { name?: string; message?: string };
  if (e?.name === "NotAllowedError") {
    return new PasskeyError("Biometric verification was cancelled or timed out.", err);
  }
  if (e?.name === "InvalidStateError") {
    return new PasskeyError("A passkey for this account already exists on this device.", err);
  }
  if (e?.name === "SecurityError") {
    return new PasskeyError("This origin isn't allowed to register passkeys (check the configured RP ID matches the site's domain).", err);
  }
  return new PasskeyError(e?.message ? `Passkey ceremony failed: ${e.message}` : "Passkey ceremony failed.", err);
}

export { bytesToHex };
