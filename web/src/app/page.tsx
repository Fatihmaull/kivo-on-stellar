"use client";

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Shield,
  Fingerprint,
  Send,
  Download,
  Users,
  CheckCircle2,
  AlertCircle,
  X,
  Plus,
  ExternalLink,
  RefreshCw,
  Wallet,
  Activity as ActivityIcon,
  Loader2,
} from "lucide-react";

import { env, explorerContractUrl, explorerTxUrl } from "@/lib/env";
import { connectWallet, disconnectWallet, fundTestnetAccount, loadNativeBalance, WalletError } from "@/lib/wallet";
import { createPasskey, PasskeyError } from "@/lib/passkey";
import {
  deploySmartAccount,
  getLatestLedger,
  getWalletConfig,
  invokeAsOwner,
  pollWalletEvents,
  type SubmitProgress,
  type TxStatus,
  type WalletConfig,
  type WalletEvent,
} from "@/lib/kivo";
import { loadWallet, saveWallet, clearWallet } from "@/lib/storage";
import { readContract, ContractCallError } from "@/lib/soroban";
import { xdr, Address, nativeToScVal } from "@stellar/stellar-sdk";

// ═══════════════════════════════════════════════════════════════════════
// Small shared pieces
// ═══════════════════════════════════════════════════════════════════════

const STATUS_LABEL: Record<TxStatus, string> = {
  building: "Preparing transaction…",
  simulating: "Simulating on Soroban RPC…",
  signing: "Waiting for signature…",
  submitting: "Broadcasting to the network…",
  pending: "Awaiting confirmation…",
  success: "Confirmed",
  failed: "Failed",
};

function TxStatusPanel({ progress }: { progress: SubmitProgress | null }) {
  if (!progress) return null;
  const isTerminal = progress.status === "success" || progress.status === "failed";
  return (
    <div
      className={`flex items-center gap-3 rounded-xl border p-3.5 text-sm ${
        progress.status === "success"
          ? "border-emerald-200 bg-emerald-50 text-emerald-700"
          : progress.status === "failed"
          ? "border-red-200 bg-red-50 text-red-700"
          : "border-blue-150 bg-blue-50 text-blue-700"
      }`}
    >
      {!isTerminal && <Loader2 className="w-4 h-4 animate-spin shrink-0" />}
      {progress.status === "success" && <CheckCircle2 className="w-4 h-4 shrink-0" />}
      {progress.status === "failed" && <AlertCircle className="w-4 h-4 shrink-0" />}
      <div className="flex-1 min-w-0">
        <p className="font-semibold">{STATUS_LABEL[progress.status]}</p>
        {progress.hash && (
          <a
            href={explorerTxUrl(progress.hash)}
            target="_blank"
            rel="noreferrer"
            className="text-xs font-mono hover:underline flex items-center gap-1 mt-0.5 truncate"
          >
            {progress.hash.substring(0, 16)}… <ExternalLink className="w-3 h-3 shrink-0" />
          </a>
        )}
      </div>
    </div>
  );
}

/** Poll for new contract events on a fixed interval, starting from
 * whatever the current ledger is when the hook mounts. This is the
 * "event listening and state synchronization" piece: no manual refresh
 * button pretending to be live data. */
function useWalletEvents(contractId: string | null) {
  const [events, setEvents] = useState<WalletEvent[]>([]);
  const cursorRef = useRef<number | null>(null);

  useEffect(() => {
    if (!contractId) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    const tick = async () => {
      try {
        if (cursorRef.current === null) {
          const latest = await getLatestLedger();
          cursorRef.current = Math.max(1, latest - 17_280); // ~last 24h
        }
        const { events: fresh, latestLedger } = await pollWalletEvents([contractId], cursorRef.current);
        if (!cancelled && fresh.length > 0) {
          setEvents((prev) => [...fresh].reverse().concat(prev).slice(0, 50));
        }
        cursorRef.current = latestLedger + 1;
      } catch {
        // Transient RPC hiccups shouldn't crash the poll loop — just retry
        // on the next tick.
      } finally {
        if (!cancelled) timer = setTimeout(tick, 6_000);
      }
    };

    tick();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [contractId]);

  return events;
}

// ═══════════════════════════════════════════════════════════════════════
// Root component
// ═══════════════════════════════════════════════════════════════════════

export default function KivoApp() {
  // Restore a previously created wallet on this device, if any — read
  // once via lazy initializers rather than an effect, since this is a
  // synchronous, one-time derivation of initial state, not a
  // subscription to an external system.
  const [restoredWallet] = useState(() => loadWallet());

  const [view, setView] = useState<"landing" | "dashboard">(() => (restoredWallet ? "dashboard" : "landing"));

  // Classic wallet (StellarWalletsKit) — pays fees, deposits funds.
  const [connectedAddress, setConnectedAddress] = useState<string | null>(null);
  const [xlmBalance, setXlmBalance] = useState<string>("0.0000");
  const [accountExists, setAccountExists] = useState(true);
  const [isConnecting, setIsConnecting] = useState(false);

  // Smart account identity (passkey + deployed contract).
  const [contractId, setContractId] = useState<string | null>(() => restoredWallet?.contractId ?? null);
  const [rawCredentialId, setRawCredentialId] = useState<Uint8Array | null>(() => restoredWallet?.rawCredentialId ?? null);
  const [credentialId32, setCredentialId32] = useState<Uint8Array | null>(() => restoredWallet?.credentialId32 ?? null);
  const [config, setConfig] = useState<WalletConfig | null>(null);
  const [smartAccountBalance, setSmartAccountBalance] = useState<string>("0");

  const [onboardingStatus, setOnboardingStatus] = useState<SubmitProgress | null>(null);
  const [isOnboarding, setIsOnboarding] = useState(false);

  const [activeTab, setActiveTab] = useState<"overview" | "send" | "deposit" | "guardians" | "activity">("overview");

  const [errorBanner, setErrorBanner] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);

  const events = useWalletEvents(contractId);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => setNotification(null), 6000);
  };
  const showError = (msg: string) => {
    setErrorBanner(msg);
    setTimeout(() => setErrorBanner(null), 8000);
  };

  const refreshConfig = useCallback(async () => {
    if (!contractId) return;
    try {
      const cfg = await getWalletConfig(contractId);
      setConfig(cfg);
      const balance = await readContract<bigint>(env.nativeSacContractId, "balance", [new Address(contractId).toScVal()]).catch(() => 0n);
      setSmartAccountBalance((Number(balance) / 10_000_000).toFixed(4));
    } catch (err) {
      if (err instanceof ContractCallError) showError(err.message);
    }
  }, [contractId]);

  useEffect(() => {
    // Genuine async network fetch (Soroban RPC simulation), not a pure
    // derivation of existing state — re-runs whenever `contractId` changes.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    refreshConfig();
  }, [refreshConfig]);

  const refreshClassicBalance = useCallback(async (address: string) => {
    try {
      const { balanceXlm, exists } = await loadNativeBalance(address);
      setXlmBalance(balanceXlm);
      setAccountExists(exists);
    } catch (err) {
      if (err instanceof WalletError) showError(err.message);
    }
  }, []);

  // ── Connect classic wallet ──
  const handleConnectWallet = async () => {
    setIsConnecting(true);
    setErrorBanner(null);
    try {
      const address = await connectWallet();
      setConnectedAddress(address);
      await refreshClassicBalance(address);
      showNotification("Wallet connected.");
    } catch (err) {
      if (err instanceof WalletError) showError(err.message);
      else showError("Failed to connect wallet.");
    } finally {
      setIsConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    await disconnectWallet();
    setConnectedAddress(null);
    setXlmBalance("0.0000");
    showNotification("Wallet disconnected.");
  };

  const handleFundTestnetAccount = async () => {
    if (!connectedAddress) return;
    try {
      await fundTestnetAccount(connectedAddress);
      await refreshClassicBalance(connectedAddress);
      showNotification("Friendbot funded your connected wallet with testnet XLM.");
    } catch (err) {
      if (err instanceof WalletError) showError(err.message);
    }
  };

  // ── Create passkey + deploy the SmartAccount ──
  const handleOnboarding = async () => {
    if (!connectedAddress) {
      showError("Connect a classic wallet first — it pays the one-time deployment fee for your new Smart Account.");
      return;
    }
    if (!accountExists) {
      showError("Your connected wallet has no testnet XLM yet. Use the faucet button first.");
      return;
    }

    setIsOnboarding(true);
    setErrorBanner(null);
    try {
      const passkey = await createPasskey(env.rpId, "Kivo Wallet", `kivo-${connectedAddress.slice(0, 6)}`);

      const rpIdHash = await (await import("@/lib/walletSignature")).sha256(new TextEncoder().encode(env.rpId));

      const { contractId: newContractId } = await deploySmartAccount(
        {
          feePayerAddress: connectedAddress,
          ownerCredentialId32: passkey.credentialId32,
          ownerPublicKey65: passkey.publicKeyPoint65,
          rpIdHash32: rpIdHash,
          recoveryThreshold: 2,
          recoveryTimelockLedgers: 34_560, // ~48h
          defaultDailyLimit: 1_000_000_0000000n,
        },
        setOnboardingStatus
      );

      saveWallet({ contractId: newContractId, rawCredentialId: passkey.rawCredentialId, credentialId32: passkey.credentialId32 });
      setContractId(newContractId);
      setRawCredentialId(passkey.rawCredentialId);
      setCredentialId32(passkey.credentialId32);
      setView("dashboard");
      showNotification("Smart Account deployed on Soroban testnet.");
    } catch (err) {
      if (err instanceof PasskeyError || err instanceof WalletError || err instanceof ContractCallError) {
        showError(err.message);
      } else {
        showError(`Failed to create your Smart Account: ${(err as Error).message ?? err}`);
      }
    } finally {
      setIsOnboarding(false);
      setTimeout(() => setOnboardingStatus(null), 4000);
    }
  };

  const resetDemo = () => {
    clearWallet();
    setView("landing");
    setContractId(null);
    setRawCredentialId(null);
    setCredentialId32(null);
    setConfig(null);
    handleDisconnect();
  };

  return (
    <div className="flex-1 flex flex-col font-sans min-h-screen bg-white">
      {errorBanner && (
        <div className="fixed bottom-6 left-6 right-6 sm:right-auto z-50 flex items-start gap-3 bg-[#cf202f] text-white px-5 py-4 rounded-2xl shadow-xl max-w-md">
          <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
          <span className="text-sm font-medium">{errorBanner}</span>
          <button onClick={() => setErrorBanner(null)} className="ml-auto shrink-0"><X className="w-4 h-4" /></button>
        </div>
      )}
      {notification && (
        <div className="fixed top-6 right-6 z-50 flex items-center gap-3 bg-neutral-900 text-white px-5 py-4 rounded-2xl shadow-xl border border-neutral-800">
          <CheckCircle2 className="w-5 h-5 text-emerald-500" />
          <span className="text-sm font-medium">{notification}</span>
        </div>
      )}

      {view === "landing" && (
        <LandingView
          connectedAddress={connectedAddress}
          xlmBalance={xlmBalance}
          accountExists={accountExists}
          isConnecting={isConnecting}
          isOnboarding={isOnboarding}
          onboardingStatus={onboardingStatus}
          onConnect={handleConnectWallet}
          onDisconnect={handleDisconnect}
          onFund={handleFundTestnetAccount}
          onCreateWallet={handleOnboarding}
        />
      )}

      {view === "dashboard" && contractId && rawCredentialId && credentialId32 && (
        <DashboardView
          contractId={contractId}
          rawCredentialId={rawCredentialId}
          credentialId32={credentialId32}
          config={config}
          smartAccountBalance={smartAccountBalance}
          connectedAddress={connectedAddress}
          xlmBalance={xlmBalance}
          activeTab={activeTab}
          setActiveTab={setActiveTab}
          events={events}
          onConnectClassic={handleConnectWallet}
          onRefresh={refreshConfig}
          onReset={resetDemo}
          showError={showError}
          showNotification={showNotification}
        />
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════
// Landing
// ═══════════════════════════════════════════════════════════════════════

function LandingView(props: {
  connectedAddress: string | null;
  xlmBalance: string;
  accountExists: boolean;
  isConnecting: boolean;
  isOnboarding: boolean;
  onboardingStatus: SubmitProgress | null;
  onConnect: () => void;
  onDisconnect: () => void;
  onFund: () => void;
  onCreateWallet: () => void;
}) {
  const { connectedAddress, xlmBalance, accountExists, isConnecting, isOnboarding, onboardingStatus, onConnect, onDisconnect, onFund, onCreateWallet } = props;

  return (
    <div className="flex-1 flex flex-col">
      <header className="max-w-[1200px] w-full mx-auto px-6 h-20 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-full bg-blue-600 flex items-center justify-center">
            <span className="text-white font-bold text-lg select-none">K</span>
          </div>
          <span className="text-xl font-bold tracking-tight text-neutral-950">Kivo</span>
        </div>

        <div className="flex items-center gap-3">
          {connectedAddress ? (
            <div className="flex items-center gap-2 bg-neutral-50 border border-neutral-200 px-4 py-2 rounded-full">
              <Wallet className="w-3.5 h-3.5 text-blue-600" />
              <span className="text-xs font-mono text-neutral-950">
                {connectedAddress.substring(0, 6)}...{connectedAddress.substring(connectedAddress.length - 4)}
              </span>
              <span className="text-xs font-mono text-neutral-500">{xlmBalance} XLM</span>
              <button onClick={onDisconnect} className="text-[10px] text-red-600 font-semibold hover:underline ml-2">
                Disconnect
              </button>
            </div>
          ) : (
            <button
              onClick={onConnect}
              disabled={isConnecting}
              className="text-xs font-semibold bg-neutral-50 hover:bg-neutral-100 disabled:opacity-60 px-4 py-2 rounded-full border border-neutral-200 transition-all inline-flex items-center gap-2"
            >
              {isConnecting && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
              Connect Wallet
            </button>
          )}
        </div>
      </header>

      <main className="flex-1 max-w-[1200px] w-full mx-auto px-6 grid grid-cols-1 lg:grid-cols-12 gap-12 items-center py-12 lg:py-20">
        <div className="lg:col-span-6 flex flex-col items-start space-y-7">
          <div className="inline-flex items-center gap-2 bg-neutral-100 px-4 py-2 rounded-full">
            <span className="w-2 h-2 rounded-full bg-blue-600 animate-pulse" />
            <span className="text-xs font-bold text-neutral-950 tracking-wide uppercase">Soroban-Native Account Abstraction</span>
          </div>

          <h1 className="text-5xl lg:text-6xl font-normal leading-tight tracking-tighter text-neutral-950">
            Your keys,
            <br />
            your biometrics.
          </h1>

          <p className="text-lg text-neutral-600 max-w-md leading-relaxed font-light">
            A real passkey creates a real Soroban smart account — deployed live on testnet, secured by
            secp256r1 signature verification on-chain. No seed phrase, ever.
          </p>

          {!connectedAddress && (
            <p className="text-sm text-neutral-500 max-w-md">
              Step 1: connect a classic wallet (Freighter, xBull, or Albedo) — it pays the one-time fee to deploy
              your Smart Account contract.
            </p>
          )}

          {connectedAddress && !accountExists && (
            <div className="w-full flex items-center gap-3 bg-amber-50 border border-amber-200 rounded-xl p-4">
              <span className="text-xs text-amber-800 flex-1">Your connected wallet has no testnet XLM yet.</span>
              <button onClick={onFund} className="text-xs font-bold text-amber-900 bg-amber-100 hover:bg-amber-200 px-3 py-1.5 rounded-full whitespace-nowrap">
                Fund via Friendbot
              </button>
            </div>
          )}

          <div className="w-full flex flex-col sm:flex-row gap-4 pt-2">
            <button
              onClick={onCreateWallet}
              disabled={isOnboarding || !connectedAddress}
              className="flex-1 sm:flex-none h-14 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-300 text-white px-8 rounded-full font-semibold transition-all duration-200 flex items-center justify-center gap-3 shadow-md"
            >
              {isOnboarding ? (
                <>
                  <RefreshCw className="w-5 h-5 animate-spin" />
                  <span>{onboardingStatus ? STATUS_LABEL[onboardingStatus.status] : "Working…"}</span>
                </>
              ) : (
                <>
                  <Fingerprint className="w-5 h-5" />
                  <span>Create Account with Passkey</span>
                </>
              )}
            </button>
          </div>

          {onboardingStatus && <TxStatusPanel progress={onboardingStatus} />}

          <div className="inline-flex items-center gap-2 text-xs font-medium text-neutral-500 tracking-widest uppercase">
            <Shield className="w-4 h-4 text-blue-600" />
            <span>Secured by Stellar Consensus · {env.network}</span>
          </div>
        </div>

        <div className="lg:col-span-6 flex justify-center items-center">
          <div className="w-full max-w-[480px] bg-neutral-950 rounded-[28px] p-6 shadow-2xl border border-neutral-850 text-white space-y-4">
            <div className="flex justify-between items-center">
              <span className="text-[10px] font-bold text-white/60 tracking-wide uppercase">What happens on-chain</span>
              <span className="bg-white/10 px-3 py-1 rounded-full text-[10px] font-semibold text-white/80 uppercase">{env.network}</span>
            </div>
            {[
              ["1", "Browser generates a real secp256r1 keypair", "Private key never leaves the device's secure enclave"],
              ["2", "You sign the deploy transaction with your classic wallet", "One-time fee — everything after this is passkey-only"],
              ["3", "A fresh SmartAccountContract instance is created", "Its __constructor registers your passkey as owner, atomically"],
            ].map(([n, t, d]) => (
              <div key={n} className="flex gap-3 bg-neutral-900 rounded-2xl p-4 border border-white/5">
                <span className="w-6 h-6 rounded-full bg-blue-600/20 text-blue-400 text-xs font-bold flex items-center justify-center shrink-0">{n}</span>
                <div>
                  <p className="text-sm font-medium">{t}</p>
                  <p className="text-xs text-white/50 mt-0.5">{d}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </main>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════════════════════

function DashboardView(props: {
  contractId: string;
  rawCredentialId: Uint8Array;
  credentialId32: Uint8Array;
  config: WalletConfig | null;
  smartAccountBalance: string;
  connectedAddress: string | null;
  xlmBalance: string;
  activeTab: "overview" | "send" | "deposit" | "guardians" | "activity";
  setActiveTab: (t: "overview" | "send" | "deposit" | "guardians" | "activity") => void;
  events: WalletEvent[];
  onConnectClassic: () => void;
  onRefresh: () => Promise<void>;
  onReset: () => void;
  showError: (msg: string) => void;
  showNotification: (msg: string) => void;
}) {
  const {
    contractId,
    rawCredentialId,
    credentialId32,
    config,
    smartAccountBalance,
    connectedAddress,
    xlmBalance,
    activeTab,
    setActiveTab,
    events,
    onConnectClassic,
    onRefresh,
    onReset,
    showError,
    showNotification,
  } = props;

  return (
    <div className="flex-1 flex flex-col bg-neutral-50">
      <header className="sticky top-0 z-30 bg-white border-b border-neutral-200 h-16">
        <div className="max-w-[1200px] w-full mx-auto px-6 h-full flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2 cursor-pointer" onClick={onReset}>
              <div className="w-7 h-7 rounded-full bg-blue-600 flex items-center justify-center">
                <span className="text-white font-bold text-sm">K</span>
              </div>
              <span className="font-bold text-lg text-neutral-950">Kivo</span>
            </div>
            <div className="h-4 w-px bg-neutral-200" />
            <div className="flex items-center gap-1.5 bg-blue-50 border border-blue-150 rounded-full px-2.5 py-1">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-600 animate-pulse" />
              <span className="text-[10px] font-bold text-blue-600 uppercase tracking-wide">Soroban {env.network}</span>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {connectedAddress ? (
              <div className="hidden sm:flex flex-col items-end">
                <span className="text-[11px] font-bold text-neutral-950 tracking-wide uppercase">Fee payer</span>
                <span className="text-xs font-mono text-neutral-500">{xlmBalance} XLM</span>
              </div>
            ) : (
              <button onClick={onConnectClassic} className="text-xs font-semibold bg-neutral-50 hover:bg-neutral-100 px-3 py-1.5 rounded-full border border-neutral-200">
                Connect classic wallet
              </button>
            )}
            <a
              href={explorerContractUrl(contractId)}
              target="_blank"
              rel="noreferrer"
              className="hidden sm:flex flex-col items-end hover:opacity-70"
            >
              <span className="text-[11px] font-bold text-neutral-950 tracking-wide uppercase">Smart Account</span>
              <span className="text-xs font-mono text-neutral-500">
                {contractId.substring(0, 6)}...{contractId.substring(contractId.length - 4)}
              </span>
            </a>
            <button onClick={onReset} className="w-9 h-9 rounded-full bg-neutral-100 border border-neutral-200 flex items-center justify-center hover:bg-neutral-200" title="Reset demo">
              <Fingerprint className="w-5 h-5 text-blue-600" />
            </button>
          </div>
        </div>
      </header>

      <main className="flex-1 max-w-[1200px] w-full mx-auto px-4 sm:px-6 py-8 grid grid-cols-1 lg:grid-cols-12 gap-8">
        <div className="lg:col-span-8 space-y-8">
          <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm">
            <div className="flex justify-between items-start mb-6">
              <div className="space-y-1">
                <span className="text-xs text-neutral-500 font-semibold tracking-wide uppercase">Smart Account Balance</span>
                <div className="text-4xl sm:text-5xl font-mono font-medium tracking-tight text-neutral-950">
                  {smartAccountBalance} <span className="text-neutral-500 text-xl font-sans font-light">XLM</span>
                </div>
              </div>
              <button onClick={onRefresh} className="w-9 h-9 rounded-full bg-neutral-100 hover:bg-neutral-200 flex items-center justify-center" title="Refresh">
                <RefreshCw className="w-4 h-4 text-neutral-600" />
              </button>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
              {[
                { id: "send", label: "Send", icon: Send },
                { id: "deposit", label: "Deposit", icon: Download },
                { id: "guardians", label: "Guardians", icon: Users },
                { id: "activity", label: "Activity", icon: ActivityIcon },
              ].map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id as typeof activeTab)}
                  className={`h-12 rounded-full font-semibold text-sm transition-all duration-200 flex items-center justify-center gap-2 ${
                    activeTab === tab.id ? "bg-blue-600 text-white shadow-md shadow-blue-600/20" : "bg-neutral-100 hover:bg-neutral-200 text-neutral-950"
                  }`}
                >
                  <tab.icon className="w-4 h-4" />
                  <span>{tab.label}</span>
                </button>
              ))}
            </div>
          </div>

          {activeTab === "overview" && <OverviewPanel config={config} contractId={contractId} events={events} />}
          {activeTab === "send" && (
            <SendPanel
              contractId={contractId}
              rawCredentialId={rawCredentialId}
              credentialId32={credentialId32}
              connectedAddress={connectedAddress}
              onSuccess={async () => {
                await onRefresh();
                showNotification("Transfer confirmed on-chain.");
              }}
              onError={showError}
            />
          )}
          {activeTab === "deposit" && (
            <DepositPanel
              contractId={contractId}
              connectedAddress={connectedAddress}
              onConnectClassic={onConnectClassic}
              onSuccess={async () => {
                await onRefresh();
                showNotification("Deposit confirmed on-chain.");
              }}
              onError={showError}
            />
          )}
          {activeTab === "guardians" && (
            <GuardiansPanel
              contractId={contractId}
              rawCredentialId={rawCredentialId}
              credentialId32={credentialId32}
              connectedAddress={connectedAddress}
              config={config}
              events={events}
              onSuccess={async () => {
                await onRefresh();
                showNotification("Guardian list updated on-chain.");
              }}
              onError={showError}
            />
          )}
          {activeTab === "activity" && <ActivityPanel events={events} />}
        </div>

        <div className="lg:col-span-4 space-y-6">
          <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm space-y-4">
            <h4 className="text-xs font-bold text-neutral-500 tracking-wider uppercase">Wallet Specs</h4>
            <div className="space-y-3 text-xs">
              <SpecRow label="Recovery threshold" value={config ? `${config.recovery_threshold} of ${config.guardian_count}` : "—"} />
              <SpecRow label="Credential epoch" value={config ? String(config.credential_epoch) : "—"} />
              <SpecRow label="Daily limit" value={config ? `${(Number(config.default_daily_limit) / 1e7).toLocaleString()} XLM` : "—"} />
              <SpecRow label="Protocol" value="Soroban / Stellar" />
            </div>
          </div>
          <div className="bg-neutral-950 rounded-3xl p-6 text-white border border-neutral-850 shadow-xl space-y-3">
            <h4 className="text-xs font-bold text-white/50 tracking-wider uppercase">Contracts</h4>
            <ContractLink label="Smart Account" id={env.smartAccountContractId} />
            <ContractLink label="Recovery Module" id={env.recoveryContractId} />
            <ContractLink label="Policy Engine" id={env.policyContractId} />
            <ContractLink label="Paymaster" id={env.paymasterContractId} />
          </div>
        </div>
      </main>
    </div>
  );
}

function SpecRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between items-center pb-2 border-b border-neutral-100 last:border-0 last:pb-0">
      <span className="text-neutral-500">{label}</span>
      <span className="font-mono font-medium text-neutral-950">{value}</span>
    </div>
  );
}

function ContractLink({ label, id }: { label: string; id: string }) {
  return (
    <a href={explorerContractUrl(id)} target="_blank" rel="noreferrer" className="flex justify-between items-center text-xs hover:opacity-70">
      <span className="text-white/50">{label}</span>
      <span className="font-mono text-white flex items-center gap-1">
        {id.substring(0, 6)}...{id.substring(id.length - 4)} <ExternalLink className="w-3 h-3" />
      </span>
    </a>
  );
}

// ── Overview ──

function OverviewPanel({ config, contractId, events }: { config: WalletConfig | null; contractId: string; events: WalletEvent[] }) {
  return (
    <div className="space-y-6">
      <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm space-y-4">
        <h3 className="text-sm font-semibold tracking-wider text-neutral-500 uppercase">Owner Credential</h3>
        {config ? (
          <div className="space-y-2 text-xs font-mono">
            <p className="text-neutral-950 break-all">credential_id: {config.owner_credential_id.toString("hex")}</p>
            <p className="text-neutral-500 break-all">rp_id_hash: {config.rp_id_hash.toString("hex")}</p>
          </div>
        ) : (
          <p className="text-sm text-neutral-500">Loading on-chain configuration…</p>
        )}
      </div>
      <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm space-y-4">
        <h3 className="text-sm font-semibold tracking-wider text-neutral-500 uppercase">Recent Activity</h3>
        {events.length === 0 ? (
          <p className="text-sm text-neutral-500">No events observed yet on this wallet. Actions you take will appear here in real time.</p>
        ) : (
          <EventList events={events.slice(0, 5)} />
        )}
      </div>
      <p className="text-xs text-neutral-400 font-mono break-all">contract: {contractId}</p>
    </div>
  );
}

// ── Send (passkey-authorized withdrawal) ──

function SendPanel(props: {
  contractId: string;
  rawCredentialId: Uint8Array;
  credentialId32: Uint8Array;
  connectedAddress: string | null;
  onSuccess: () => void;
  onError: (msg: string) => void;
}) {
  const { contractId, rawCredentialId, credentialId32, connectedAddress, onSuccess, onError } = props;
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [progress, setProgress] = useState<SubmitProgress | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connectedAddress) {
      onError("Connect a classic wallet to pay the network fee for this transaction (the transfer itself is authorized by your passkey).");
      return;
    }
    const amt = Number(amount);
    if (!recipient || !amt || amt <= 0) return;

    setSubmitting(true);
    setProgress(null);
    try {
      const args = [
        new Address(contractId).toScVal(),
        new Address(recipient).toScVal(),
        nativeToScVal(BigInt(Math.round(amt * 1e7)), { type: "i128" }),
      ];
      const result = await invokeAsOwner(
        { contractId: env.nativeSacContractId, feePayerAddress: connectedAddress, method: "transfer", args, passkey: { rawCredentialId, credentialId32 } },
        setProgress
      );
      setRecipient("");
      setAmount("");
      onSuccess();
      void result;
    } catch (err) {
      onError(describeSendError(err));
    } finally {
      setSubmitting(false);
      setTimeout(() => setProgress(null), 6000);
    }
  };

  return (
    <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm">
      <div className="flex justify-between items-center mb-6">
        <h3 className="text-base font-semibold text-neutral-950">Send XLM (passkey-authorized)</h3>
      </div>

      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="space-y-2">
          <label className="text-xs font-semibold text-neutral-500 tracking-wider uppercase">Recipient Address</label>
          <input
            type="text"
            required
            value={recipient}
            onChange={(e) => setRecipient(e.target.value)}
            placeholder="G… or C… Stellar address"
            className="w-full h-12 px-4 rounded-xl border border-neutral-200 focus:border-blue-600 focus:ring-1 focus:ring-blue-600 focus:outline-none text-sm font-mono bg-white text-neutral-950"
          />
        </div>
        <div className="space-y-2">
          <label className="text-xs font-semibold text-neutral-500 tracking-wider uppercase">Amount (XLM)</label>
          <input
            type="number"
            required
            min="0.0000001"
            step="any"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.00"
            className="w-full h-12 px-4 rounded-xl border border-neutral-200 focus:border-blue-600 focus:ring-1 focus:ring-blue-600 focus:outline-none text-sm font-mono bg-white text-neutral-950"
          />
        </div>

        <button
          type="submit"
          disabled={submitting}
          className="w-full h-12 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-300 text-white rounded-full font-semibold transition-all flex items-center justify-center gap-2"
        >
          <Fingerprint className="w-4 h-4" />
          <span>{submitting ? "Waiting for biometric verification…" : "Authorize Transfer with Passkey"}</span>
        </button>
      </form>

      {progress && <div className="mt-4"><TxStatusPanel progress={progress} /></div>}
    </div>
  );
}

function describeSendError(err: unknown): string {
  if (err instanceof PasskeyError) return err.message;
  if (err instanceof WalletError) return err.message;
  if (err instanceof ContractCallError) {
    if (err.message.includes("SpendingLimitExceeded")) return "This transfer would exceed your daily spending limit.";
    return err.message;
  }
  return `Transfer failed: ${(err as Error)?.message ?? err}`;
}

// ── Deposit (classic-wallet-authorized, no passkey needed) ──

function DepositPanel(props: {
  contractId: string;
  connectedAddress: string | null;
  onConnectClassic: () => void;
  onSuccess: () => void;
  onError: (msg: string) => void;
}) {
  const { contractId, connectedAddress, onConnectClassic, onSuccess, onError } = props;
  const [amount, setAmount] = useState("");
  const [progress, setProgress] = useState<SubmitProgress | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connectedAddress) {
      onConnectClassic();
      return;
    }
    const amt = Number(amount);
    if (!amt || amt <= 0) return;

    setSubmitting(true);
    setProgress(null);
    try {
      // Depositing is authorized by the CLASSIC wallet (it's `from` in this
      // transfer), so this never touches the passkey signing path — a
      // plain StellarWalletsKit-signed transaction is enough.
      const { server, networkPassphrase } = await import("@/lib/soroban");
      const { Contract, TransactionBuilder, BASE_FEE, rpc } = await import("@stellar/stellar-sdk");
      const { signTransactionXdr } = await import("@/lib/wallet");
      const { waitForTransaction } = await import("@/lib/soroban");

      const s = server();
      const account = await s.getAccount(connectedAddress);
      const contract = new Contract(env.nativeSacContractId);
      const args = [
        new Address(connectedAddress).toScVal(),
        new Address(contractId).toScVal(),
        nativeToScVal(BigInt(Math.round(amt * 1e7)), { type: "i128" }),
      ];
      let tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: networkPassphrase() })
        .addOperation(contract.call("transfer", ...args))
        .setTimeout(60)
        .build();

      setProgress({ status: "simulating" });
      const sim = await s.simulateTransaction(tx);
      if (rpc.Api.isSimulationError(sim)) throw new ContractCallError(sim.error);
      tx = rpc.assembleTransaction(tx, sim).build();

      setProgress({ status: "signing" });
      const signedXdr = await signTransactionXdr(tx.toXDR(), connectedAddress);
      const signedTx = TransactionBuilder.fromXDR(signedXdr, networkPassphrase());

      setProgress({ status: "submitting" });
      const sendResult = await s.sendTransaction(signedTx as never);
      if (sendResult.status === "ERROR") throw new ContractCallError("Transaction rejected before submission.");

      const final = await waitForTransaction(sendResult.hash, (status) => setProgress({ status, hash: sendResult.hash }));
      if (final.status !== rpc.Api.GetTransactionStatus.SUCCESS) throw new ContractCallError("Deposit failed on-chain.");

      setAmount("");
      onSuccess();
    } catch (err) {
      onError(describeSendError(err));
    } finally {
      setSubmitting(false);
      setTimeout(() => setProgress(null), 6000);
    }
  };

  return (
    <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm">
      <h3 className="text-base font-semibold text-neutral-950 mb-2">Deposit XLM into your Smart Account</h3>
      <p className="text-sm text-neutral-500 mb-6">
        Sent from your connected classic wallet — this is a standard signature, no passkey required.
      </p>
      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="space-y-2">
          <label className="text-xs font-semibold text-neutral-500 tracking-wider uppercase">Amount (XLM)</label>
          <input
            type="number"
            required
            min="0.0000001"
            step="any"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.00"
            className="w-full h-12 px-4 rounded-xl border border-neutral-200 focus:border-blue-600 focus:ring-1 focus:ring-blue-600 focus:outline-none text-sm font-mono bg-white text-neutral-950"
          />
        </div>
        <button
          type="submit"
          disabled={submitting}
          className="w-full h-12 bg-neutral-900 hover:bg-neutral-800 disabled:bg-neutral-400 text-white rounded-full font-semibold transition-all flex items-center justify-center gap-2"
        >
          <Download className="w-4 h-4" />
          <span>{connectedAddress ? (submitting ? "Submitting…" : "Deposit") : "Connect classic wallet"}</span>
        </button>
      </form>
      {progress && <div className="mt-4"><TxStatusPanel progress={progress} /></div>}
    </div>
  );
}

// ── Guardians ──

function GuardiansPanel(props: {
  contractId: string;
  rawCredentialId: Uint8Array;
  credentialId32: Uint8Array;
  connectedAddress: string | null;
  config: WalletConfig | null;
  events: WalletEvent[];
  onSuccess: () => void;
  onError: (msg: string) => void;
}) {
  const { contractId, rawCredentialId, credentialId32, connectedAddress, config, events, onSuccess, onError } = props;
  const [alias, setAlias] = useState("");
  const [address, setAddress] = useState("");
  const [isAdding, setIsAdding] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [progress, setProgress] = useState<SubmitProgress | null>(null);

  // Reconstruct the guardian set from on-chain events — there is no
  // "list guardians" contract method (Soroban storage maps aren't
  // enumerable by design), so the event log is the source of truth for
  // what to *show*; `is_guardian` on-chain remains the source of truth for
  // what's actually *authorized*. A pure derivation from `events`, so this
  // is a memo, not a fetch — no effect needed.
  const guardians = useMemo(() => {
    const added = new Map<string, string>();
    for (const ev of events) {
      const topic0 = Array.isArray(ev.topics) ? ev.topics[0] : undefined;
      if (topic0 === "guardian_added" && Array.isArray(ev.data)) {
        const [addr, aliasVal] = ev.data as [string, string];
        added.set(addr, aliasVal);
      }
      if (topic0 === "guardian_removed" && Array.isArray(ev.data)) {
        const [addr] = ev.data as [string];
        added.delete(addr);
      }
    }
    return Array.from(added.entries()).map(([addr, a]) => ({ address: addr, alias: a }));
  }, [events]);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connectedAddress) {
      onError("Connect a classic wallet to pay the network fee for this transaction.");
      return;
    }
    if (!alias || !address) return;

    setSubmitting(true);
    try {
      const args = [
        xdr.ScVal.scvMap(
          [
            ["address", new Address(address).toScVal()],
            ["added_at", nativeToScVal(0n, { type: "u64" })],
            ["alias", xdr.ScVal.scvSymbol(alias.slice(0, 32))],
          ]
            .sort(([a], [b]) => (a < b ? -1 : 1))
            .map(([k, v]) => new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol(k as string), val: v as xdr.ScVal }))
        ),
      ];
      await invokeAsOwner(
        { contractId, feePayerAddress: connectedAddress, method: "add_guardian", args, passkey: { rawCredentialId, credentialId32 } },
        setProgress
      );
      setAlias("");
      setAddress("");
      setIsAdding(false);
      onSuccess();
    } catch (err) {
      onError(describeSendError(err));
    } finally {
      setSubmitting(false);
      setTimeout(() => setProgress(null), 6000);
    }
  };

  return (
    <div className="space-y-6">
      <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm space-y-6">
        <div className="flex justify-between items-center">
          <div>
            <h3 className="text-base font-semibold text-neutral-950">Social Recovery Guardians</h3>
            <p className="text-xs text-neutral-500 mt-1">
              {config ? `${config.recovery_threshold}-of-${config.guardian_count} approvals rotate your owner credential after a 48h timelock.` : "Loading…"}
            </p>
          </div>
          <Users className="w-5 h-5 text-blue-600" />
        </div>

        <div className="space-y-3">
          <div className="flex justify-between items-center">
            <span className="text-xs font-semibold text-neutral-500 tracking-wider uppercase">Guardians (from on-chain events)</span>
            <button onClick={() => setIsAdding(!isAdding)} className="inline-flex items-center gap-1 text-xs font-bold text-blue-600 hover:text-blue-700">
              <Plus className="w-3.5 h-3.5" /> Add Guardian
            </button>
          </div>

          {isAdding && (
            <form onSubmit={handleAdd} className="p-4 border border-blue-300 rounded-2xl space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <input
                  type="text"
                  required
                  placeholder="Alias (max 32 chars)"
                  value={alias}
                  onChange={(e) => setAlias(e.target.value)}
                  className="h-10 px-3 rounded-lg border border-neutral-200 focus:border-blue-600 focus:outline-none text-xs bg-white text-neutral-950"
                />
                <input
                  type="text"
                  required
                  placeholder="Guardian G… address"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  className="h-10 px-3 rounded-lg border border-neutral-200 focus:border-blue-600 focus:outline-none text-xs font-mono bg-white text-neutral-950"
                />
              </div>
              <div className="flex justify-end gap-2">
                <button type="button" onClick={() => setIsAdding(false)} className="h-9 px-4 rounded-full hover:bg-neutral-100 text-xs font-semibold text-neutral-500">
                  Cancel
                </button>
                <button type="submit" disabled={submitting} className="h-9 px-4 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-300 text-white rounded-full text-xs font-semibold inline-flex items-center gap-2">
                  <Fingerprint className="w-3.5 h-3.5" />
                  {submitting ? "Signing…" : "Confirm with Passkey"}
                </button>
              </div>
              {progress && <TxStatusPanel progress={progress} />}
            </form>
          )}

          {guardians.length === 0 ? (
            <p className="text-xs text-neutral-500 py-2">
              No guardians observed yet. Add one above, or check back — events are only visible from roughly the
              last 24h of ledgers on this feed.
            </p>
          ) : (
            <div className="space-y-2">
              {guardians.map((g) => (
                <div key={g.address} className="flex items-center justify-between p-3.5 bg-white border border-neutral-200 rounded-2xl">
                  <div>
                    <p className="text-xs font-semibold text-neutral-950">{g.alias}</p>
                    <p className="text-[10px] text-neutral-500 font-mono break-all">{g.address}</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Activity ──

function ActivityPanel({ events }: { events: WalletEvent[] }) {
  return (
    <div className="bg-white rounded-3xl p-6 border border-neutral-200 shadow-sm space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold tracking-wider text-neutral-500 uppercase">Live Contract Events</h3>
        <span className="flex items-center gap-1.5 text-[10px] font-bold text-emerald-600 bg-emerald-50 px-2.5 py-1 rounded-full">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" /> Polling every 6s
        </span>
      </div>
      {events.length === 0 ? (
        <p className="text-sm text-neutral-500">No events yet. This feed polls `getEvents` on the deployed contract and updates automatically.</p>
      ) : (
        <EventList events={events} />
      )}
    </div>
  );
}

function EventList({ events }: { events: WalletEvent[] }) {
  return (
    <div className="space-y-2">
      {events.map((ev) => (
        <div key={ev.id} className="flex items-center justify-between py-2.5 border-b border-neutral-100 last:border-0">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-8 h-8 rounded-full bg-blue-50 flex items-center justify-center text-blue-600 shrink-0">
              <ActivityIcon className="w-3.5 h-3.5" />
            </div>
            <div className="min-w-0">
              <p className="text-xs font-semibold text-neutral-950 truncate">{String(Array.isArray(ev.topics) ? ev.topics[0] : "event")}</p>
              <a href={explorerTxUrl(ev.txHash)} target="_blank" rel="noreferrer" className="text-[10px] text-blue-600 font-mono hover:underline">
                ledger {ev.ledger}
              </a>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
