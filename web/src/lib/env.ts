/**
 * Central, typed access to the deployment configuration. Throws early and
 * loudly if a required contract address is missing, instead of letting a
 * `undefined` silently propagate into an RPC call and fail with an opaque
 * XDR parsing error three layers down.
 */

function required(name: string, value: string | undefined): string {
  if (!value) {
    throw new Error(
      `Missing required environment variable ${name}. Copy web/.env.example to web/.env.local and fill in your deployed contract addresses.`
    );
  }
  return value;
}

export const env = {
  network: (process.env.NEXT_PUBLIC_STELLAR_NETWORK ?? "TESTNET") as "TESTNET" | "PUBLIC",
  horizonUrl: process.env.NEXT_PUBLIC_HORIZON_URL ?? "https://horizon-testnet.stellar.org",
  sorobanRpcUrl: process.env.NEXT_PUBLIC_SOROBAN_RPC_URL ?? "https://soroban-testnet.stellar.org",
  rpId: process.env.NEXT_PUBLIC_RP_ID ?? (typeof window !== "undefined" ? window.location.hostname : "localhost"),

  get smartAccountContractId(): string {
    return required("NEXT_PUBLIC_SMART_ACCOUNT_CONTRACT_ID", process.env.NEXT_PUBLIC_SMART_ACCOUNT_CONTRACT_ID);
  },
  get recoveryContractId(): string {
    return required("NEXT_PUBLIC_RECOVERY_CONTRACT_ID", process.env.NEXT_PUBLIC_RECOVERY_CONTRACT_ID);
  },
  get policyContractId(): string {
    return required("NEXT_PUBLIC_POLICY_CONTRACT_ID", process.env.NEXT_PUBLIC_POLICY_CONTRACT_ID);
  },
  get paymasterContractId(): string {
    return required("NEXT_PUBLIC_PAYMASTER_CONTRACT_ID", process.env.NEXT_PUBLIC_PAYMASTER_CONTRACT_ID);
  },
  get nativeSacContractId(): string {
    return required("NEXT_PUBLIC_NATIVE_SAC_CONTRACT_ID", process.env.NEXT_PUBLIC_NATIVE_SAC_CONTRACT_ID);
  },
} as const;

export function explorerTxUrl(hash: string): string {
  const net = env.network === "PUBLIC" ? "public" : "testnet";
  return `https://stellar.expert/explorer/${net}/tx/${hash}`;
}

export function explorerContractUrl(id: string): string {
  const net = env.network === "PUBLIC" ? "public" : "testnet";
  return `https://stellar.expert/explorer/${net}/contract/${id}`;
}
