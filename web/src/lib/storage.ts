import { bytesToHex, hexToBytes } from "./walletSignature";

/** Client-side "which wallet is this browser's passkey for" cache.
 *
 * There is no backend index mapping a WebAuthn credential to a contract
 * address — a real deployment would run one (or rely on guardians for
 * recovery, which is what the recovery module is for). For this frontend,
 * remembering the pairing locally is the honest, standard "keep me signed
 * in on this device" pattern; losing it is exactly the scenario social
 * recovery exists to solve, not a bug to route around.
 */
export interface StoredWallet {
  contractId: string;
  rawCredentialIdHex: string;
  credentialId32Hex: string;
  createdAt: number;
}

const KEY = "kivo:wallet:v1";

export function saveWallet(w: { contractId: string; rawCredentialId: Uint8Array; credentialId32: Uint8Array }) {
  if (typeof window === "undefined") return;
  const record: StoredWallet = {
    contractId: w.contractId,
    rawCredentialIdHex: bytesToHex(w.rawCredentialId),
    credentialId32Hex: bytesToHex(w.credentialId32),
    createdAt: Date.now(),
  };
  window.localStorage.setItem(KEY, JSON.stringify(record));
}

export function loadWallet(): { contractId: string; rawCredentialId: Uint8Array; credentialId32: Uint8Array } | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(KEY);
  if (!raw) return null;
  try {
    const record = JSON.parse(raw) as StoredWallet;
    return {
      contractId: record.contractId,
      rawCredentialId: hexToBytes(record.rawCredentialIdHex),
      credentialId32: hexToBytes(record.credentialId32Hex),
    };
  } catch {
    return null;
  }
}

export function clearWallet() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(KEY);
}
