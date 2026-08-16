import { xdr } from "@stellar/stellar-sdk";

/**
 * Builds the `WalletSignature` ScVal the SmartAccount contract's
 * `__check_auth` expects — matching `crates/novus-types/src/signer.rs`
 * field-for-field.
 *
 * Soroban's `#[contracttype]` derive represents a struct as an `ScMap`
 * whose entries are the field names (as `ScSymbol`) in **sorted order**,
 * each paired with its encoded value. That ordering is not cosmetic: the
 * host decodes the map positionally against the struct's spec, so if this
 * list drifts from alphabetical (or the Rust struct gains/loses a field),
 * `__check_auth` fails to decode the argument at all rather than
 * misbehaving loudly — keep this in lockstep with `WalletSignature` in Rust.
 */
export function buildWalletSignatureScVal(fields: {
  credentialId: Uint8Array; // 32 bytes
  signatureBytes: Uint8Array; // 64 bytes
  authenticatorData: Uint8Array; // passkey only; empty for ed25519/session
  clientDataJson: Uint8Array; // passkey only; empty for ed25519/session
  nonce: bigint;
}): xdr.ScVal {
  const entries = [
    ["authenticator_data", xdr.ScVal.scvBytes(Buffer.from(fields.authenticatorData))],
    ["client_data_json", xdr.ScVal.scvBytes(Buffer.from(fields.clientDataJson))],
    ["credential_id", xdr.ScVal.scvBytes(Buffer.from(fields.credentialId))],
    ["nonce", u128ToScVal(fields.nonce)],
    ["signature_bytes", xdr.ScVal.scvBytes(Buffer.from(fields.signatureBytes))],
  ] as const;

  return xdr.ScVal.scvMap(
    entries.map(
      ([key, val]) =>
        new xdr.ScMapEntry({
          key: xdr.ScVal.scvSymbol(key),
          val,
        })
    )
  );
}

function u128ToScVal(value: bigint): xdr.ScVal {
  const hi = value >> 64n;
  const lo = value & 0xffffffffffffffffn;
  return xdr.ScVal.scvU128(
    new xdr.UInt128Parts({
      hi: xdr.Uint64.fromString(hi.toString()),
      lo: xdr.Uint64.fromString(lo.toString()),
    })
  );
}

/** P-256 (secp256r1) curve order, needed to normalize a signature to low-S. */
const P256_N = BigInt("0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551");

/**
 * Decode a DER-encoded ECDSA signature (what `AuthenticatorAssertionResponse.signature`
 * always is) into the fixed 64-byte `r ‖ s` format Soroban's `secp256r1_verify`
 * requires, normalizing `s` to the curve's lower half — the host rejects
 * "high-S" signatures, which is one of the two valid encodings any
 * ECDSA signer may produce.
 */
export function derSignatureToRaw(der: Uint8Array): Uint8Array {
  let offset = 0;
  if (der[offset++] !== 0x30) throw new Error("Not a DER SEQUENCE");
  offset += der[offset] & 0x80 ? (der[offset] & 0x7f) + 1 : 1; // skip length byte(s)

  const readInt = (): bigint => {
    if (der[offset++] !== 0x02) throw new Error("Expected DER INTEGER");
    let len = der[offset++];
    if (len & 0x80) {
      const nBytes = len & 0x7f;
      len = 0;
      for (let i = 0; i < nBytes; i++) len = (len << 8) | der[offset++];
    }
    let value = 0n;
    for (let i = 0; i < len; i++) value = (value << 8n) | BigInt(der[offset++]);
    return value;
  };

  const r = readInt();
  let s = readInt();

  if (s > P256_N / 2n) {
    s = P256_N - s;
  }

  const out = new Uint8Array(64);
  writeBigIntBE(r, out, 0, 32);
  writeBigIntBE(s, out, 32, 32);
  return out;
}

function writeBigIntBE(value: bigint, out: Uint8Array, offset: number, length: number) {
  let v = value;
  for (let i = length - 1; i >= 0; i--) {
    out[offset + i] = Number(v & 0xffn);
    v >>= 8n;
  }
}

/** SHA-256, used both for deriving a stable 32-byte credential_id from a
 * (potentially much longer) real WebAuthn `rawId`, and available generally
 * wherever the signing flow needs a digest. */
export async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(data).buffer as ArrayBuffer);
  return new Uint8Array(digest);
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }
  return out;
}
