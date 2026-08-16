import { Horizon } from "@stellar/stellar-sdk";
import { env } from "./env";

export class WalletError extends Error {
  constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = "WalletError";
  }
}

let kitPromise: Promise<typeof import("@creit.tech/stellar-wallets-kit")> | null = null;

/** Lazily load the kit and its wallet modules client-side only — the kit
 * touches `window`/`document` at import time, which breaks Next.js SSR. */
async function loadKit() {
  if (!kitPromise) {
    kitPromise = (async () => {
      const [kit, freighter, xbull, albedo] = await Promise.all([
        import("@creit.tech/stellar-wallets-kit"),
        import("@creit.tech/stellar-wallets-kit/modules/freighter"),
        import("@creit.tech/stellar-wallets-kit/modules/xbull"),
        import("@creit.tech/stellar-wallets-kit/modules/albedo"),
      ]);
      kit.StellarWalletsKit.init({
        network: env.network === "PUBLIC" ? kit.Networks.PUBLIC : kit.Networks.TESTNET,
        modules: [new freighter.FreighterModule(), new xbull.xBullModule(), new albedo.AlbedoModule()],
      });
      return kit;
    })();
  }
  return kitPromise;
}

/** Opens the kit's own wallet-selection modal (Freighter / xBull / Albedo),
 * connects, and returns the connected G-address. Throws a `WalletError`
 * with a message fit for direct display for the three cases Level 2/3
 * explicitly call out: extension missing, user rejection, and any other
 * connection failure. */
export async function connectWallet(): Promise<string> {
  const kit = await loadKit();
  try {
    const { address } = await kit.StellarWalletsKit.authModal();
    return address;
  } catch (err) {
    throw classifyWalletError(err);
  }
}

export async function disconnectWallet(): Promise<void> {
  const kit = await loadKit();
  await kit.StellarWalletsKit.disconnect();
}

export async function signTransactionXdr(xdrString: string, address: string): Promise<string> {
  const kit = await loadKit();
  try {
    const { signedTxXdr } = await kit.StellarWalletsKit.signTransaction(xdrString, {
      address,
      networkPassphrase: env.network === "PUBLIC" ? "Public Global Stellar Network ; September 2015" : "Test SDF Network ; September 2015",
    });
    return signedTxXdr;
  } catch (err) {
    throw classifyWalletError(err);
  }
}

function classifyWalletError(err: unknown): WalletError {
  const message = (err as { message?: string; toString?: () => string })?.message ?? String(err);
  const lower = message.toLowerCase();
  if (lower.includes("not found") || lower.includes("not installed") || lower.includes("no modules")) {
    return new WalletError("No Stellar wallet extension was found. Install Freighter, xBull, or Albedo and try again.", err);
  }
  if (lower.includes("reject") || lower.includes("declined") || lower.includes("cancel") || lower.includes("user closed")) {
    return new WalletError("Connection request was rejected.", err);
  }
  return new WalletError(`Wallet connection failed: ${message}`, err);
}

export interface NativeBalance {
  balanceXlm: string;
  exists: boolean;
}

/** Loads XLM balance via Horizon. A 404 means the classic account has
 * never been funded — surfaced distinctly so the UI can offer the testnet
 * faucet instead of a generic error. */
export async function loadNativeBalance(address: string): Promise<NativeBalance> {
  const horizon = new Horizon.Server(env.horizonUrl);
  try {
    const account = await horizon.loadAccount(address);
    const native = account.balances.find((b) => b.asset_type === "native");
    return { balanceXlm: native ? Number(native.balance).toFixed(4) : "0.0000", exists: true };
  } catch (err) {
    const status = (err as { response?: { status?: number } })?.response?.status;
    if (status === 404) {
      return { balanceXlm: "0.0000", exists: false };
    }
    throw new WalletError("Failed to load account balance from Horizon.", err);
  }
}

/** Testnet-only: request friendbot funding for a freshly connected
 * classic account with zero balance, so onboarding doesn't dead-end. */
export async function fundTestnetAccount(address: string): Promise<void> {
  if (env.network !== "TESTNET") {
    throw new WalletError("Friendbot funding is only available on testnet.");
  }
  const res = await fetch(`https://friendbot.stellar.org?addr=${encodeURIComponent(address)}`);
  if (!res.ok) {
    throw new WalletError(`Friendbot funding failed (HTTP ${res.status}).`);
  }
}
