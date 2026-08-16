import {
  Address,
  BASE_FEE,
  Contract,
  Operation,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
  type Transaction,
} from "@stellar/stellar-sdk";
import { env, explorerContractUrl } from "./env";
import {
  ContractCallError,
  authorizeSmartAccountEntries,
  describeContractError,
  networkPassphrase,
  readContract,
  server,
  waitForTransaction,
  type SubmitProgress,
  type TxStatus,
} from "./soroban";
import { signPasskeyAuth } from "./passkey";
import { signTransactionXdr } from "./wallet";

// ═══════════════════════════════════════════════════════════════════════
// READS
// ═══════════════════════════════════════════════════════════════════════

export interface WalletConfig {
  version: number;
  owner_credential_id: Buffer;
  rp_id_hash: Buffer;
  recovery_threshold: number;
  recovery_timelock_ledgers: number;
  guardian_count: number;
  default_daily_limit: bigint;
  credential_epoch: number;
}

export async function getWalletConfig(contractId: string): Promise<WalletConfig | null> {
  try {
    return await readContract<WalletConfig>(contractId, "get_config");
  } catch (err) {
    if (err instanceof ContractCallError && err.message.includes("NotInitialized")) return null;
    throw err;
  }
}

export async function getNonce(contractId: string): Promise<bigint> {
  return readContract<bigint>(contractId, "get_nonce");
}

export async function isSigner(contractId: string, credentialId32: Uint8Array): Promise<boolean> {
  return readContract<boolean>(contractId, "is_signer", [xdr.ScVal.scvBytes(Buffer.from(credentialId32))]);
}

export async function isGuardian(contractId: string, address: string): Promise<boolean> {
  return readContract<boolean>(contractId, "is_guardian", [new Address(address).toScVal()]);
}

export async function getGuardian(contractId: string, address: string) {
  return readContract<{ address: string; added_at: bigint; alias: string }>(contractId, "get_guardian", [
    new Address(address).toScVal(),
  ]);
}

export async function getSession(contractId: string, sessionId: Uint8Array) {
  return readContract(contractId, "get_session", [xdr.ScVal.scvBytes(Buffer.from(sessionId))]);
}

// ═══════════════════════════════════════════════════════════════════════
// EVENTS — real-time activity feed / state sync
// ═══════════════════════════════════════════════════════════════════════

export interface WalletEvent {
  id: string;
  ledger: number;
  ledgerClosedAt: string;
  topics: unknown[];
  data: unknown;
  txHash: string;
}

/** Poll for new contract events since `startLedger`. Call again with
 * `result.latestLedger + 1` as the next `startLedger` to keep a live feed
 * moving forward without re-fetching what's already been seen. */
export async function pollWalletEvents(
  contractIds: string[],
  startLedger: number
): Promise<{ events: WalletEvent[]; latestLedger: number }> {
  const s = server();
  const res = await s.getEvents({
    startLedger,
    filters: [{ type: "contract", contractIds }],
    limit: 50,
  });
  const events: WalletEvent[] = res.events.map((e) => ({
    id: e.id,
    ledger: e.ledger,
    ledgerClosedAt: e.ledgerClosedAt,
    topics: e.topic.map((t) => scValToNative(t)),
    data: scValToNative(e.value),
    txHash: e.txHash,
  }));
  return { events, latestLedger: res.latestLedger };
}

export async function getLatestLedger(): Promise<number> {
  const res = await server().getLatestLedger();
  return res.sequence;
}

// ═══════════════════════════════════════════════════════════════════════
// DEPLOYMENT — real on-chain contract creation, paid for by the connected
// classic wallet (fee sponsorship needs a live backend relayer; this is
// the honest "self-sponsored first deploy" path for a frontend with none)
// ═══════════════════════════════════════════════════════════════════════

export interface DeployParams {
  feePayerAddress: string;
  ownerCredentialId32: Uint8Array;
  ownerPublicKey65: Uint8Array;
  rpIdHash32: Uint8Array;
  recoveryThreshold: number;
  recoveryTimelockLedgers: number;
  defaultDailyLimit: bigint;
}

export async function deploySmartAccount(
  params: DeployParams,
  onProgress?: (p: SubmitProgress) => void
): Promise<{ contractId: string; txHash: string }> {
  onProgress?.({ status: "building" });
  const s = server();

  const instance = await s.getContractInstance(env.smartAccountContractId);
  const wasmHash = instance.executable().wasmHash();

  const constructorArgs = [
    xdr.ScVal.scvBytes(Buffer.from(params.ownerCredentialId32)),
    xdr.ScVal.scvBytes(Buffer.from(params.ownerPublicKey65)),
    xdr.ScVal.scvBytes(Buffer.from(params.rpIdHash32)),
    xdr.ScVal.scvU32(params.recoveryThreshold),
    xdr.ScVal.scvU32(params.recoveryTimelockLedgers),
    nativeToScVal(params.defaultDailyLimit, { type: "i128" }),
  ];

  const salt = crypto.getRandomValues(new Uint8Array(32));
  const op = Operation.createCustomContract({
    address: new Address(params.feePayerAddress),
    wasmHash,
    constructorArgs,
    salt,
  });

  const account = await s.getAccount(params.feePayerAddress);
  let tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: networkPassphrase() })
    .addOperation(op)
    .setTimeout(60)
    .build();

  onProgress?.({ status: "simulating" });
  const sim = await s.simulateTransaction(tx);
  const { rpc } = await import("@stellar/stellar-sdk");
  if (rpc.Api.isSimulationError(sim)) {
    throw new ContractCallError(describeContractError(sim.error), sim);
  }
  tx = rpc.assembleTransaction(tx, sim).build();

  onProgress?.({ status: "signing" });
  const signedXdr = await signTransactionXdr(tx.toXDR(), params.feePayerAddress);
  const signedTx = TransactionBuilder.fromXDR(signedXdr, networkPassphrase()) as Transaction;

  onProgress?.({ status: "submitting" });
  const sendResult = await s.sendTransaction(signedTx);
  if (sendResult.status === "ERROR") {
    throw new ContractCallError("Transaction rejected before submission.", sendResult);
  }

  const finalResult = await waitForTransaction(sendResult.hash, (status) => onProgress?.({ status, hash: sendResult.hash }));
  if (finalResult.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new ContractCallError("Deployment transaction failed on-chain.", finalResult);
  }

  const contractId = scValToNative(finalResult.returnValue!) as string;
  return { contractId, txHash: sendResult.hash };
}

// ═══════════════════════════════════════════════════════════════════════
// WRITES — invoke a SmartAccount method, authorized by a real passkey
// assertion, fee-paid by the connected classic wallet.
// ═══════════════════════════════════════════════════════════════════════

export interface PasskeyIdentity {
  rawCredentialId: Uint8Array;
  credentialId32: Uint8Array;
}

export async function invokeAsOwner(
  params: {
    contractId: string;
    feePayerAddress: string;
    method: string;
    args: xdr.ScVal[];
    passkey: PasskeyIdentity;
  },
  onProgress?: (p: SubmitProgress) => void
): Promise<{ txHash: string; returnValue: unknown }> {
  onProgress?.({ status: "building" });
  const s = server();
  const account = await s.getAccount(params.feePayerAddress);
  const contract = new Contract(params.contractId);

  let tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: networkPassphrase() })
    .addOperation(contract.call(params.method, ...params.args))
    .setTimeout(60)
    .build();

  onProgress?.({ status: "simulating" });
  const sim = await s.simulateTransaction(tx);
  const { rpc } = await import("@stellar/stellar-sdk");
  if (rpc.Api.isSimulationError(sim)) {
    throw new ContractCallError(describeContractError(sim.error), sim);
  }
  const latestLedger = await s.getLatestLedger();
  const validUntilLedgerSeq = latestLedger.sequence + 200; // ~15-20 min

  tx = rpc.assembleTransaction(tx, sim).build();

  onProgress?.({ status: "signing" });
  // The nonce this signature must carry is whatever the contract's
  // sequential counter is *right now* — read it fresh rather than trusting
  // stale UI state, since a stale nonce here just means the signature
  // won't verify at all.
  const nonce = await getNonce(params.contractId);

  await authorizeSmartAccountEntries(
    tx,
    params.contractId,
    async (_preimage, payload) => signPasskeyAuth(params.passkey.rawCredentialId, params.passkey.credentialId32, nonce, payload),
    validUntilLedgerSeq
  );

  const preparedSim = await s.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(preparedSim)) {
    throw new ContractCallError(describeContractError(preparedSim.error), preparedSim);
  }
  tx = rpc.assembleTransaction(tx, preparedSim).build();

  const signedXdr = await signTransactionXdr(tx.toXDR(), params.feePayerAddress);
  const signedTx = TransactionBuilder.fromXDR(signedXdr, networkPassphrase()) as Transaction;

  onProgress?.({ status: "submitting" });
  const sendResult = await s.sendTransaction(signedTx);
  if (sendResult.status === "ERROR") {
    throw new ContractCallError("Transaction rejected before submission.", sendResult);
  }

  const finalResult = await waitForTransaction(sendResult.hash, (status) => onProgress?.({ status, hash: sendResult.hash }));
  if (finalResult.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    const raw = "resultXdr" in finalResult ? String(finalResult.resultXdr) : JSON.stringify(finalResult);
    throw new ContractCallError(`Transaction failed on-chain: ${raw}`, finalResult);
  }

  return {
    txHash: sendResult.hash,
    returnValue: finalResult.returnValue ? scValToNative(finalResult.returnValue) : undefined,
  };
}

export type { SubmitProgress, TxStatus };
export { explorerContractUrl };
