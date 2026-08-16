import {
  Account,
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  Networks,
  TransactionBuilder,
  authorizeEntry,
  rpc,
  scValToNative,
  xdr,
  type Transaction,
} from "@stellar/stellar-sdk";
import { env } from "./env";

export function server(): rpc.Server {
  return new rpc.Server(env.sorobanRpcUrl, { allowHttp: env.sorobanRpcUrl.startsWith("http://") });
}

export function networkPassphrase(): string {
  return env.network === "PUBLIC" ? Networks.PUBLIC : Networks.TESTNET;
}

/**
 * Simulation needs a syntactically valid source account to build a
 * transaction envelope, but a read-only `simulateTransaction` call never
 * touches real ledger state for that account and is never submitted — so a
 * throwaway local keypair with a fixed sequence number is enough. This
 * avoids requiring a connected wallet just to read `get_config`.
 */
const READ_ONLY_SOURCE = Keypair.random();

export class ContractCallError extends Error {
  constructor(message: string, public readonly raw?: unknown) {
    super(message);
    this.name = "ContractCallError";
  }
}

/** Decode a Soroban simulation's raw diagnostic text into a readable message.
 * Contract errors surface as `Error(Contract, #<code>)` — map the codes we
 * export from `WalletError` back to their names so failures are legible
 * instead of a bare number. */
const WALLET_ERROR_NAMES: Record<number, string> = {
  100: "NotInitialized",
  101: "AlreadyInitialized",
  200: "InvalidSignature",
  201: "InvalidNonce",
  202: "SignerNotFound",
  204: "SignerAlreadyExists",
  205: "ReplayDetected",
  206: "InvalidAuthContext",
  210: "ChallengeMismatch",
  211: "WrongCeremonyType",
  212: "RpIdMismatch",
  213: "UserPresenceMissing",
  214: "UserVerificationMissing",
  215: "MalformedClientData",
  216: "MalformedAssertion",
  217: "StaleCredentialEpoch",
  218: "InsufficientWeight",
  300: "PolicyViolation",
  301: "SpendingLimitExceeded",
  302: "UnauthorizedTarget",
  303: "UnauthorizedFunction",
  304: "AmountExceedsSessionCap",
  400: "RecoveryNotReady",
  401: "TimelockActive",
  402: "InsufficientGuardians",
  403: "DuplicateGuardian",
  404: "GuardianNotFound",
  405: "ProposalNotFound",
  406: "ProposalAlreadyExecuted",
  407: "ProposalCancelled",
  409: "BelowRecoveryThreshold",
  500: "SessionKeyInvalid",
  501: "SessionKeyExpired",
  502: "SessionTotalExceeded",
  600: "UnsupportedFeeToken",
  601: "InsufficientFeeBalance",
  603: "FeeExceedsMax",
  900: "Unauthorized",
  999: "InternalError",
};

export function describeContractError(raw: string): string {
  const match = raw.match(/Error\(Contract,\s*#(\d+)\)/);
  if (match) {
    const code = Number(match[1]);
    const name = WALLET_ERROR_NAMES[code];
    return name ? `${name} (contract error #${code})` : `Contract error #${code}`;
  }
  return raw;
}

/** Read a contract method with no authorization required. */
export async function readContract<T>(contractId: string, method: string, args: xdr.ScVal[] = []): Promise<T> {
  const s = server();
  const account = new Account(READ_ONLY_SOURCE.publicKey(), "0");
  const contract = new Contract(contractId);
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: networkPassphrase() })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const sim = await s.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) {
    throw new ContractCallError(describeContractError(sim.error), sim);
  }
  if (!rpc.Api.isSimulationSuccess(sim) || !sim.result) {
    throw new ContractCallError("Simulation did not return a result", sim);
  }
  return scValToNative(sim.result.retval) as T;
}

export type TxStatus = "building" | "simulating" | "signing" | "submitting" | "pending" | "success" | "failed";

export interface SubmitProgress {
  status: TxStatus;
  hash?: string;
}

/**
 * Sign a transaction's Soroban auth entries with a custom signing callback
 * for entries authorized by `authorizingAddress` (the SmartAccount
 * contract), leaving any other entries (e.g. a classic wallet's own
 * source-account authorization) untouched for the caller to sign
 * separately.
 */
export async function authorizeSmartAccountEntries(
  tx: Transaction,
  authorizingAddress: string,
  sign: (preimage: xdr.HashIdPreimage, payload: Buffer) => Promise<xdr.ScVal>,
  validUntilLedgerSeq: number
): Promise<void> {
  const op = tx.operations[0];
  if (op.type !== "invokeHostFunction" || !op.auth) return;

  for (let i = 0; i < op.auth.length; i++) {
    const entry = op.auth[i];
    const credentials = entry.credentials();
    if (credentials.switch().name !== "sorobanCredentialsAddress") continue;
    const addr = credentials.address().address();
    if (addr.switch().name !== "scAddressTypeContract") continue;

    const contractAddress = new Address(authorizingAddress);
    if (!contractAddress.toScAddress().toXDR().equals(addr.toXDR())) continue;

    const signed = await authorizeEntry(
      entry,
      async (preimage: xdr.HashIdPreimage, payload: Buffer) => ({
        signatureScVal: await sign(preimage, payload),
      }),
      validUntilLedgerSeq,
      networkPassphrase()
    );
    op.auth[i] = signed;
  }
}

/** Poll `getTransaction` until the network reports a terminal status. */
export async function waitForTransaction(
  hash: string,
  onUpdate?: (status: TxStatus) => void,
  timeoutMs = 45_000
): Promise<rpc.Api.GetTransactionResponse> {
  const s = server();
  const start = Date.now();
  onUpdate?.("pending");
  for (;;) {
    const res = await s.getTransaction(hash);
    if (res.status !== rpc.Api.GetTransactionStatus.NOT_FOUND) {
      onUpdate?.(res.status === rpc.Api.GetTransactionStatus.SUCCESS ? "success" : "failed");
      return res;
    }
    if (Date.now() - start > timeoutMs) {
      throw new ContractCallError(`Transaction ${hash} was not confirmed within ${timeoutMs}ms`);
    }
    await new Promise((r) => setTimeout(r, 1500));
  }
}
