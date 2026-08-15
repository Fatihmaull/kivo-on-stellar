"use client";

import React, { useState, useEffect } from "react";
import {
  Shield,
  Key,
  Fingerprint,
  Send,
  Download,
  Sliders,
  Users,
  CheckCircle2,
  AlertCircle,
  X,
  Plus,
  ArrowRight,
  TrendingUp,
  CircleDot,
  Trash2,
  ExternalLink,
  ChevronRight,
  Cpu,
  RefreshCw,
} from "lucide-react";

// Types matching contracts
interface SessionKeyItem {
  id: string;
  name: string;
  contract: string;
  limit: string;
  spent: string;
  expires: string;
}

interface GuardianItem {
  address: string;
  alias: string;
  addedAt: string;
}

export default function KivoApp() {
  // Navigation states: 'landing', 'dashboard'
  const [view, setView] = useState<"landing" | "dashboard">("landing");

  // Onboarding States
  const [isOnboarding, setIsOnboarding] = useState(false);
  const [onboardingStep, setOnboardingStep] = useState(0);
  const [createdWalletAddress, setCreatedWalletAddress] = useState("");

  // Wallet States
  const [balance, setBalance] = useState(12450.8);
  const [dailyLimit, setDailyLimit] = useState(5000);
  const [tempLimit, setTempLimit] = useState(5000);
  
  // Guardians State
  const [guardians, setGuardians] = useState<GuardianItem[]>([
    { address: "GD3W...74KA", alias: "Alice (Phone)", addedAt: "Aug 15, 2026" },
    { address: "GB2E...93LD", alias: "Ledger Backup", addedAt: "Aug 15, 2026" },
  ]);
  const [newGuardianName, setNewGuardianName] = useState("");
  const [newGuardianAddress, setNewGuardianAddress] = useState("");
  const [isAddingGuardian, setIsAddingGuardian] = useState(false);

  // Session Keys State
  const [sessions, setSessions] = useState<SessionKeyItem[]>([
    {
      id: "sess_01",
      name: "StellarX Swap",
      contract: "CA3W...KIVO",
      limit: "500 USDC",
      spent: "120 USDC",
      expires: "24h left",
    },
    {
      id: "sess_02",
      name: "Soroswap Pool",
      contract: "CB8R...SWAP",
      limit: "1000 USDC",
      spent: "0 USDC",
      expires: "6d left",
    },
  ]);

  // Active Sub-Panel in Dashboard: 'overview', 'transfer', 'policy', 'guardians'
  const [activeTab, setActiveTab] = useState<"overview" | "transfer" | "policy" | "guardians">("overview");

  // Transfer Form States
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [gasToken, setGasToken] = useState<"FREE" | "USDC" | "XLM" | "EURC">("FREE");
  const [transferStep, setTransferStep] = useState<"idle" | "signing" | "broadcasting" | "success">("idle");
  const [txHash, setTxHash] = useState("");

  // Notification Banner
  const [notification, setNotification] = useState<string | null>(null);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => setNotification(null), 5000);
  };

  // Onboarding simulation
  const handleOnboarding = () => {
    setIsOnboarding(true);
    setOnboardingStep(1);

    // Step 1: WebAuthn Trigger
    setTimeout(() => {
      setOnboardingStep(2); // Generating passkey
      setTimeout(() => {
        setOnboardingStep(3); // Deploying Soroban Smart Contract
        // Generate mock wallet contract address
        const randomAddr = "G" + Array.from({ length: 55 }, () =>
          "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"[Math.floor(Math.random() * 32)]
        ).join("");
        setCreatedWalletAddress(randomAddr);
        
        setTimeout(() => {
          setIsOnboarding(false);
          setView("dashboard");
          showNotification("Smart Account Wallet deployed successfully on Soroban!");
        }, 2000);
      }, 1800);
    }, 1500);
  };

  // Submit transfer simulation
  const handleTransfer = (e: React.FormEvent) => {
    e.preventDefault();
    if (!recipient || !amount) return;

    setTransferStep("signing");

    // Simulate Passkey Signing prompt
    setTimeout(() => {
      setTransferStep("broadcasting");

      // Simulate Soroban Relayer Fee-Bump Submission
      setTimeout(() => {
        setTransferStep("success");
        const mockHash = Math.random().toString(16).substring(2, 10) + "..." + Math.random().toString(16).substring(2, 6);
        setTxHash(mockHash);
        
        // Deduct balance
        const transferAmt = parseFloat(amount);
        setBalance((prev) => prev - transferAmt);
        showNotification(`Successfully sent ${amount} USDC to ${recipient.substring(0, 8)}...`);
      }, 2000);
    }, 1800);
  };

  // Revoke session key
  const handleRevokeSession = (id: string, name: string) => {
    setSessions((prev) => prev.filter((s) => s.id !== id));
    showNotification(`Session key for ${name} revoked.`);
  };

  // Add Guardian
  const handleAddGuardian = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newGuardianName || !newGuardianAddress) return;

    const newGuardian: GuardianItem = {
      address: newGuardianAddress.length > 12 ? `${newGuardianAddress.substring(0, 4)}...${newGuardianAddress.substring(newGuardianAddress.length - 4)}` : newGuardianAddress,
      alias: newGuardianName,
      addedAt: "Just now",
    };

    setGuardians((prev) => [...prev, newGuardian]);
    setNewGuardianName("");
    setNewGuardianAddress("");
    setIsAddingGuardian(false);
    showNotification(`Guardian ${newGuardianName} added successfully.`);
  };

  // Reset demo
  const resetDemo = () => {
    setView("landing");
    setBalance(12450.8);
    setDailyLimit(5000);
    setTempLimit(5000);
    setTransferStep("idle");
    setRecipient("");
    setAmount("");
    setSessions([
      {
        id: "sess_01",
        name: "StellarX Swap",
        contract: "CA3W...KIVO",
        limit: "500 USDC",
        spent: "120 USDC",
        expires: "24h left",
      },
      {
        id: "sess_02",
        name: "Soroswap Pool",
        contract: "CB8R...SWAP",
        limit: "1000 USDC",
        spent: "0 USDC",
        expires: "6d left",
      },
    ]);
    setGuardians([
      { address: "GD3W...74KA", alias: "Alice (Phone)", addedAt: "Aug 15, 2026" },
      { address: "GB2E...93LD", alias: "Ledger Backup", addedAt: "Aug 15, 2026" },
    ]);
  };

  return (
    <div className="flex-1 flex flex-col font-sans selection:bg-kivo-blue/10 selection:text-kivo-blue min-h-screen bg-white">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed top-6 right-6 z-50 flex items-center gap-3 bg-surface-dark text-white px-5 py-4 rounded-2xl shadow-xl border border-surface-dark-elevated transition-all duration-300 animate-in fade-in slide-in-from-top-4">
          <CheckCircle2 className="w-5 h-5 text-semantic-green" />
          <span className="text-sm font-medium">{notification}</span>
        </div>
      )}

      {/* ──────────────────────────────────────────────────────── */}
      {/* LANDING PAGE VIEW */}
      {/* ──────────────────────────────────────────────────────── */}
      {view === "landing" && (
        <div className="flex-1 flex flex-col">
          {/* Header */}
          <header className="max-w-[1200px] w-full mx-auto px-6 h-20 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-kivo-blue flex items-center justify-center">
                <span className="text-white font-bold text-lg select-none">K</span>
              </div>
              <span className="text-xl font-bold tracking-tight text-ink-strong">Kivo</span>
            </div>
            <div className="flex items-center gap-4">
              <span className="text-xs font-semibold tracking-wider text-muted-gray uppercase bg-surface-strong px-3 py-1.5 rounded-full select-none">
                Stellar Journey Flagship
              </span>
            </div>
          </header>

          {/* Main Hero Grid */}
          <main className="flex-1 max-w-[1200px] w-full mx-auto px-6 grid grid-cols-1 lg:grid-cols-12 gap-12 items-center py-12 lg:py-24">
            {/* Left Column: Headline & Value Prop */}
            <div className="lg:col-span-5 flex flex-col items-start space-y-8">
              <div className="inline-flex items-center gap-2 bg-surface-strong px-4 py-2 rounded-full">
                <CircleDot className="w-3.5 h-3.5 text-kivo-blue animate-pulse" />
                <span className="text-xs font-bold text-ink-strong tracking-wide uppercase">
                  Soroban-Native Account Abstraction
                </span>
              </div>
              
              <h1 className="text-5xl lg:text-7xl font-normal leading-tight tracking-tighter text-ink-strong display-tracking">
                Your keys,<br />your biometrics.
              </h1>

              <p className="text-lg text-body-gray max-w-md leading-relaxed font-light">
                Kivo combines non-custodial security with the simplicity of Web2. Zero seed phrases, custom policy controls, and 100% sponsored gas payments.
              </p>

              <div className="w-full flex flex-col sm:flex-row gap-4 pt-4">
                <button
                  onClick={handleOnboarding}
                  disabled={isOnboarding}
                  className="flex-1 sm:flex-none h-14 bg-kivo-blue hover:bg-kivo-blue-hover disabled:bg-kivo-blue-disabled text-white px-8 rounded-full font-semibold transition-all duration-200 flex items-center justify-center gap-3 shadow-md hover:shadow-lg active:scale-98"
                >
                  {isOnboarding ? (
                    <>
                      <RefreshCw className="w-5 h-5 animate-spin" />
                      <span>Creating wallet...</span>
                    </>
                  ) : (
                    <>
                      <Fingerprint className="w-5 h-5" />
                      <span>Create Account with Passkey</span>
                    </>
                  )}
                </button>
              </div>

              <div className="inline-flex items-center gap-2 text-xs font-medium text-muted-gray tracking-widest uppercase">
                <Shield className="w-4 h-4 text-kivo-blue" />
                <span>Secured by Stellar Consensus</span>
              </div>
            </div>

            {/* Right Column: Layered Dashboard Mockup */}
            <div className="lg:col-span-7 flex justify-center items-center">
              <div className="w-full max-w-[520px] bg-surface-dark rounded-[32px] p-6 shadow-2xl border border-surface-dark-elevated text-white overflow-hidden relative">
                {/* Simulated Device Top Bar */}
                <div className="flex justify-between items-center mb-6 px-2">
                  <div className="flex items-center gap-2">
                    <span className="w-2.5 h-2.5 rounded-full bg-white/20"></span>
                    <span className="w-2.5 h-2.5 rounded-full bg-white/20"></span>
                  </div>
                  <div className="bg-white/10 px-3 py-1 rounded-full text-[10px] font-semibold text-white/80 tracking-wider uppercase">
                    Testnet Wallet Demo
                  </div>
                </div>

                {/* Dashboard Header */}
                <div className="bg-surface-dark-elevated rounded-2xl p-5 border border-white/5 mb-5">
                  <div className="flex justify-between items-start mb-2">
                    <span className="text-xs text-white/50 font-medium">TOTAL BALANCE</span>
                    <span className="text-[10px] font-bold text-semantic-green bg-semantic-green/10 px-2 py-0.5 rounded-full flex items-center gap-1 select-none">
                      <TrendingUp className="w-2.5 h-2.5" /> +2.4%
                    </span>
                  </div>
                  <div className="text-3xl font-mono font-medium tracking-tight mb-4 text-white">
                    $12,450.80<span className="text-white/40 text-lg ml-1 font-sans">USDC</span>
                  </div>
                  
                  {/* Gas sponsored banner */}
                  <div className="flex items-center gap-2 bg-kivo-blue/15 border border-kivo-blue/30 rounded-xl p-3">
                    <Cpu className="w-4 h-4 text-kivo-blue" />
                    <span className="text-[11px] font-semibold text-kivo-blue tracking-wide uppercase">
                      Gasless Enabled (Sponsored by Paymaster)
                    </span>
                  </div>
                </div>

                {/* Quick actions grid preview */}
                <div className="grid grid-cols-4 gap-3 mb-6">
                  {["Send", "Receive", "Limits", "Guardians"].map((label, idx) => (
                    <div key={label} className="bg-surface-dark-elevated rounded-xl p-3 flex flex-col items-center justify-center gap-2 border border-white/5 cursor-not-allowed hover:bg-white/5 transition-colors">
                      {idx === 0 && <Send className="w-4 h-4 text-white/60" />}
                      {idx === 1 && <Download className="w-4 h-4 text-white/60" />}
                      {idx === 2 && <Sliders className="w-4 h-4 text-white/60" />}
                      {idx === 3 && <Users className="w-4 h-4 text-white/60" />}
                      <span className="text-[11px] text-white/60 font-medium">{label}</span>
                    </div>
                  ))}
                </div>

                {/* Simulated Policy Center preview card */}
                <div className="bg-surface-dark-elevated rounded-2xl p-4 border border-white/5 space-y-4">
                  <div className="flex justify-between items-center">
                    <span className="text-xs text-white/70 font-semibold tracking-wide uppercase">Security Rules</span>
                    <span className="text-[10px] text-kivo-blue font-bold bg-kivo-blue/10 px-2 py-0.5 rounded-full">ACTIVE</span>
                  </div>
                  <div className="space-y-3">
                    <div className="flex justify-between text-xs text-white/50">
                      <span>Daily Transfer Limit</span>
                      <span className="font-mono text-white">$5,000 USDC</span>
                    </div>
                    <div className="w-full bg-white/10 h-1 rounded-full overflow-hidden">
                      <div className="bg-kivo-blue h-full w-[70%]"></div>
                    </div>
                    <div className="flex justify-between items-center pt-1 border-t border-white/5 text-[11px] text-white/40">
                      <span>Guardians Configured</span>
                      <span className="text-white">2 of 3 Quorum</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </main>

          {/* Onboarding Dialog Sheet overlay */}
          {isOnboarding && (
            <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
              <div className="w-full max-w-md bg-white rounded-3xl p-8 shadow-2xl border border-hairline flex flex-col items-center text-center space-y-6 animate-in zoom-in-95">
                <div className="w-16 h-16 rounded-full bg-surface-strong flex items-center justify-center text-kivo-blue relative">
                  {onboardingStep === 1 && <Fingerprint className="w-8 h-8 animate-pulse" />}
                  {onboardingStep === 2 && <Key className="w-8 h-8 animate-bounce" />}
                  {onboardingStep === 3 && <Cpu className="w-8 h-8 animate-spin" />}
                </div>

                <div className="space-y-2">
                  <h3 className="text-xl font-semibold text-ink-strong">
                    {onboardingStep === 1 && "Simulating Biometric Authentication"}
                    {onboardingStep === 2 && "Registering Passkey Credentials"}
                    {onboardingStep === 3 && "Deploying Soroban Contract Account"}
                  </h3>
                  <p className="text-sm text-body-gray max-w-xs mx-auto">
                    {onboardingStep === 1 && "Please approve the system WebAuthn challenge with your FaceID / TouchID."}
                    {onboardingStep === 2 && "Generating secp256r1 keys inside your device's Secure Enclave."}
                    {onboardingStep === 3 && "Creating your autonomous, paymaster-sponsored account contract."}
                  </p>
                </div>

                <div className="w-full bg-surface-strong h-1.5 rounded-full overflow-hidden">
                  <div
                    className="bg-kivo-blue h-full transition-all duration-1000"
                    style={{
                      width:
                        onboardingStep === 1
                          ? "33%"
                          : onboardingStep === 2
                          ? "66%"
                          : "100%",
                    }}
                  ></div>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ──────────────────────────────────────────────────────── */}
      {/* APP DASHBOARD VIEW */}
      {/* ──────────────────────────────────────────────────────── */}
      {view === "dashboard" && (
        <div className="flex-1 flex flex-col bg-surface-soft">
          {/* Sticky Nav Header */}
          <header className="sticky top-0 z-30 bg-white border-b border-hairline h-16">
            <div className="max-w-[1200px] w-full mx-auto px-6 h-full flex items-center justify-between">
              {/* Logo / Network */}
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2 cursor-pointer" onClick={resetDemo}>
                  <div className="w-7 h-7 rounded-full bg-kivo-blue flex items-center justify-center">
                    <span className="text-white font-bold text-sm">K</span>
                  </div>
                  <span className="font-bold text-lg text-ink-strong">Kivo</span>
                </div>
                <div className="h-4 w-px bg-hairline"></div>
                <div className="flex items-center gap-1.5 bg-kivo-blue/10 border border-kivo-blue/20 rounded-full px-2.5 py-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-kivo-blue animate-pulse"></span>
                  <span className="text-[10px] font-bold text-kivo-blue uppercase tracking-wide">Soroban Testnet</span>
                </div>
              </div>

              {/* Account Address Badge */}
              <div className="flex items-center gap-3">
                <div className="hidden sm:flex flex-col items-end">
                  <span className="text-[11px] font-bold text-ink-strong tracking-wide uppercase">Smart Account</span>
                  <span className="text-xs font-mono text-body-gray">{createdWalletAddress.substring(0, 6)}...{createdWalletAddress.substring(createdWalletAddress.length - 4)}</span>
                </div>
                
                {/* User avatar / profile button */}
                <button
                  onClick={resetDemo}
                  className="w-9 h-9 rounded-full bg-surface-strong border border-hairline flex items-center justify-center hover:bg-surface-strong/80 transition-colors"
                  title="Disconnect Wallet"
                >
                  <Fingerprint className="w-5 h-5 text-kivo-blue" />
                </button>
              </div>
            </div>
          </header>

          {/* Main Dashboard Layout */}
          <main className="flex-1 max-w-[1200px] w-full mx-auto px-6 py-8 grid grid-cols-1 lg:grid-cols-12 gap-8">
            {/* Left Columns: Tabs & Details */}
            <div className="lg:col-span-8 space-y-8">
              {/* Hero Balance Card */}
              <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm relative overflow-hidden">
                <div className="flex justify-between items-start mb-3">
                  <div className="space-y-1">
                    <span className="text-xs text-body-gray font-semibold tracking-wide uppercase">AVAILABLE PORTFOLIO VALUE</span>
                    <div className="text-4xl sm:text-5xl font-mono font-medium tracking-tight text-ink-strong">
                      ${balance.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} <span className="text-body-gray text-2xl font-sans font-light">USDC</span>
                    </div>
                  </div>
                  <div className="text-right">
                    <span className="text-xs font-bold text-semantic-green bg-semantic-green/10 px-3 py-1 rounded-full inline-flex items-center gap-1 select-none">
                      <TrendingUp className="w-3 h-3" /> +2.4%
                    </span>
                    <p className="text-[11px] text-muted-gray mt-1">24h Gain</p>
                  </div>
                </div>

                {/* Gas Status / Paymaster sponsorship banner */}
                <div className="flex items-center gap-2 bg-kivo-blue/10 border border-kivo-blue/20 rounded-xl p-3.5 mb-6">
                  <div className="w-2 h-2 rounded-full bg-kivo-blue animate-pulse"></div>
                  <span className="text-xs font-semibold text-kivo-blue tracking-wide uppercase">
                    Gasless Enabled (Sponsored by Paymaster)
                  </span>
                </div>

                {/* Quick actions triggers */}
                <div className="grid grid-cols-4 gap-3">
                  <button
                    onClick={() => { setActiveTab("transfer"); setTransferStep("idle"); }}
                    className={`h-12 rounded-full font-semibold text-sm transition-all duration-200 flex items-center justify-center gap-2 ${
                      activeTab === "transfer"
                        ? "bg-kivo-blue text-white shadow-md shadow-kivo-blue/20"
                        : "bg-surface-strong hover:bg-surface-strong/80 text-ink-strong"
                    }`}
                  >
                    <Send className="w-4 h-4" />
                    <span className="hidden sm:inline">Send Assets</span>
                  </button>

                  <button
                    onClick={() => setActiveTab("overview")}
                    className={`h-12 rounded-full font-semibold text-sm transition-all duration-200 flex items-center justify-center gap-2 ${
                      activeTab === "overview"
                        ? "bg-kivo-blue text-white shadow-md shadow-kivo-blue/20"
                        : "bg-surface-strong hover:bg-surface-strong/80 text-ink-strong"
                    }`}
                  >
                    <Download className="w-4 h-4" />
                    <span className="hidden sm:inline">Receive</span>
                  </button>

                  <button
                    onClick={() => setActiveTab("policy")}
                    className={`h-12 rounded-full font-semibold text-sm transition-all duration-200 flex items-center justify-center gap-2 ${
                      activeTab === "policy"
                        ? "bg-kivo-blue text-white shadow-md shadow-kivo-blue/20"
                        : "bg-surface-strong hover:bg-surface-strong/80 text-ink-strong"
                    }`}
                  >
                    <Sliders className="w-4 h-4" />
                    <span className="hidden sm:inline">Policy & limits</span>
                  </button>

                  <button
                    onClick={() => setActiveTab("guardians")}
                    className={`h-12 rounded-full font-semibold text-sm transition-all duration-200 flex items-center justify-center gap-2 ${
                      activeTab === "guardians"
                        ? "bg-kivo-blue text-white shadow-md shadow-kivo-blue/20"
                        : "bg-surface-strong hover:bg-surface-strong/80 text-ink-strong"
                    }`}
                  >
                    <Users className="w-4 h-4" />
                    <span className="hidden sm:inline">Guardians</span>
                  </button>
                </div>
              </div>

              {/* ── SUB-PANEL: OVERVIEW / DEFAULT ── */}
              {activeTab === "overview" && (
                <div className="space-y-6">
                  {/* Asset Allocation */}
                  <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-4">
                    <h3 className="text-sm font-semibold tracking-wider text-muted-gray uppercase">Asset Allocations</h3>
                    
                    <div className="space-y-3">
                      {/* USDC token row */}
                      <div className="flex items-center justify-between py-2.5 border-b border-hairline">
                        <div className="flex items-center gap-3">
                          <div className="w-10 h-10 rounded-full bg-[#2775ca]/10 border border-[#2775ca]/20 flex items-center justify-center">
                            <span className="text-[#2775ca] font-bold text-sm">USDC</span>
                          </div>
                          <div>
                            <p className="text-sm font-semibold text-ink-strong">USD Coin</p>
                            <p className="text-xs text-body-gray">Soroban Asset Contract</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-mono font-medium text-ink-strong">${balance.toLocaleString("en-US", { minimumFractionDigits: 2 })}</p>
                          <p className="text-xs text-muted-gray">{balance.toLocaleString()} USDC</p>
                        </div>
                      </div>

                      {/* XLM token row */}
                      <div className="flex items-center justify-between py-2.5">
                        <div className="flex items-center gap-3">
                          <div className="w-10 h-10 rounded-full bg-surface-strong border border-hairline flex items-center justify-center">
                            <span className="text-ink-strong font-bold text-sm">XLM</span>
                          </div>
                          <div>
                            <p className="text-sm font-semibold text-ink-strong">Stellar Lumens</p>
                            <p className="text-xs text-body-gray">Native Gas / Asset</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-mono font-medium text-ink-strong">$0.00</p>
                          <p className="text-xs text-muted-gray">0.00 XLM</p>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Transaction History Mock */}
                  <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-4">
                    <h3 className="text-sm font-semibold tracking-wider text-muted-gray uppercase">Recent Activity</h3>
                    
                    <div className="space-y-3">
                      {txHash ? (
                        <div className="flex items-center justify-between py-3 border-b border-hairline">
                          <div className="flex items-center gap-3">
                            <div className="w-9 h-9 rounded-full bg-semantic-green/10 flex items-center justify-center text-semantic-green">
                              <Send className="w-4 h-4" />
                            </div>
                            <div>
                              <p className="text-sm font-semibold text-ink-strong">Sent USDC</p>
                              <p className="text-xs text-body-gray font-mono">Hash: {txHash}</p>
                            </div>
                          </div>
                          <div className="text-right">
                            <p className="text-sm font-mono font-medium text-semantic-red">-{amount} USDC</p>
                            <p className="text-[10px] text-muted-gray">Just now</p>
                          </div>
                        </div>
                      ) : null}

                      <div className="flex items-center justify-between py-3 border-b border-hairline">
                        <div className="flex items-center gap-3">
                          <div className="w-9 h-9 rounded-full bg-semantic-green/10 flex items-center justify-center text-semantic-green">
                            <Download className="w-4 h-4" />
                          </div>
                          <div>
                            <p className="text-sm font-semibold text-ink-strong">Received USDC</p>
                            <p className="text-xs text-body-gray">From GA5W...98LD</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-mono font-medium text-semantic-green">+$12,450.80</p>
                          <p className="text-[10px] text-muted-gray">Aug 15, 2026</p>
                        </div>
                      </div>

                      <div className="flex items-center justify-between py-3">
                        <div className="flex items-center gap-3">
                          <div className="w-9 h-9 rounded-full bg-kivo-blue/10 flex items-center justify-center text-kivo-blue">
                            <Cpu className="w-4 h-4" />
                          </div>
                          <div>
                            <p className="text-sm font-semibold text-ink-strong">Wallet Deployed</p>
                            <p className="text-xs text-body-gray">Soroban initialization complete</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-semibold text-kivo-blue text-xs uppercase bg-kivo-blue/10 px-2 py-0.5 rounded-full">Success</p>
                          <p className="text-[10px] text-muted-gray">Aug 15, 2026</p>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* ── SUB-PANEL: TRANSFER ── */}
              {activeTab === "transfer" && (
                <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm">
                  <div className="flex justify-between items-center mb-6">
                    <h3 className="text-base font-semibold text-ink-strong">Gasless Token Transfer</h3>
                    <button onClick={() => setActiveTab("overview")} className="text-muted-gray hover:text-ink-strong">
                      <X className="w-5 h-5" />
                    </button>
                  </div>

                  {transferStep === "idle" && (
                    <form onSubmit={handleTransfer} className="space-y-6">
                      {/* Recipient Address */}
                      <div className="space-y-2">
                        <label className="text-xs font-semibold text-body-gray tracking-wider uppercase">Recipient Address</label>
                        <input
                          type="text"
                          required
                          value={recipient}
                          onChange={(e) => setRecipient(e.target.value)}
                          placeholder="Stellar address (e.g. GB2E...93LD)"
                          className="w-full h-12 px-4 rounded-xl border border-hairline focus:border-kivo-blue focus:ring-1 focus:ring-kivo-blue focus:outline-none transition-all duration-200 text-sm font-mono bg-white text-ink-strong"
                        />
                      </div>

                      {/* Amount */}
                      <div className="space-y-2">
                        <label className="text-xs font-semibold text-body-gray tracking-wider uppercase">Amount (USDC)</label>
                        <div className="relative">
                          <input
                            type="number"
                            required
                            min="0.01"
                            step="any"
                            value={amount}
                            onChange={(e) => setAmount(e.target.value)}
                            placeholder="0.00"
                            className="w-full h-12 px-4 pr-16 rounded-xl border border-hairline focus:border-kivo-blue focus:ring-1 focus:ring-kivo-blue focus:outline-none transition-all duration-200 text-sm font-mono bg-white text-ink-strong"
                          />
                          <span className="absolute right-4 top-3 text-xs font-bold text-muted-gray uppercase select-none">USDC</span>
                        </div>
                      </div>

                      {/* Gas Token Selector */}
                      <div className="space-y-2">
                        <label className="text-xs font-semibold text-body-gray tracking-wider uppercase">Pay Gas/Network Fee With</label>
                        <div className="grid grid-cols-4 gap-2">
                          {[
                            { value: "FREE", label: "Sponsored", desc: "100% Free" },
                            { value: "USDC", label: "USDC", desc: "Pay with token" },
                            { value: "XLM", label: "XLM", desc: "Pay with native" },
                            { value: "EURC", label: "EURC", desc: "Pay with token" },
                          ].map((token) => (
                            <button
                              key={token.value}
                              type="button"
                              onClick={() => setGasToken(token.value as any)}
                              className={`p-3 rounded-xl border text-center transition-all duration-150 flex flex-col justify-center items-center ${
                                gasToken === token.value
                                  ? "border-kivo-blue bg-kivo-blue/5 text-kivo-blue font-semibold"
                                  : "border-hairline hover:bg-surface-soft text-body-gray"
                              }`}
                            >
                              <span className="text-xs font-bold">{token.label}</span>
                              <span className="text-[9px] opacity-70 mt-0.5">{token.desc}</span>
                            </button>
                          ))}
                        </div>
                      </div>

                      {/* Submit */}
                      <button
                        type="submit"
                        className="w-full h-12 bg-kivo-blue hover:bg-kivo-blue-hover text-white rounded-full font-semibold transition-all duration-200 flex items-center justify-center gap-2"
                      >
                        <Fingerprint className="w-4 h-4" />
                        <span>Authorize Transfer with Passkey</span>
                      </button>
                    </form>
                  )}

                  {/* Simulated Passkey Signing */}
                  {transferStep === "signing" && (
                    <div className="py-12 flex flex-col items-center justify-center text-center space-y-6">
                      <div className="w-16 h-16 rounded-full bg-surface-strong flex items-center justify-center text-kivo-blue relative animate-pulse">
                        <Fingerprint className="w-8 h-8" />
                      </div>
                      <div className="space-y-2">
                        <h4 className="text-lg font-semibold text-ink-strong">Simulating Passkey Approval</h4>
                        <p className="text-xs text-body-gray max-w-xs mx-auto">
                          Signing the SorobanAuthorizationEntry with your private key from device secure storage.
                        </p>
                      </div>
                    </div>
                  )}

                  {/* Simulated Transaction Broadcasting */}
                  {transferStep === "broadcasting" && (
                    <div className="py-12 flex flex-col items-center justify-center text-center space-y-6">
                      <div className="w-16 h-16 rounded-full bg-kivo-blue/10 flex items-center justify-center text-kivo-blue relative animate-spin">
                        <RefreshCw className="w-8 h-8" />
                      </div>
                      <div className="space-y-2">
                        <h4 className="text-lg font-semibold text-ink-strong">Broadcasting to Stellar Testnet</h4>
                        <p className="text-xs text-body-gray max-w-xs mx-auto">
                          Relayer wrapped the UserOperation into a Fee-Bump Transaction and submitted to Soroban RPC.
                        </p>
                      </div>
                    </div>
                  )}

                  {/* Transfer Success */}
                  {transferStep === "success" && (
                    <div className="py-12 flex flex-col items-center justify-center text-center space-y-6">
                      <div className="w-16 h-16 rounded-full bg-semantic-green/10 flex items-center justify-center text-semantic-green">
                        <CheckCircle2 className="w-8 h-8" />
                      </div>
                      <div className="space-y-2">
                        <h4 className="text-lg font-semibold text-ink-strong">Transfer Sent successfully!</h4>
                        <p className="text-xs text-body-gray max-w-xs mx-auto">
                          Gas fees sponsored by Kivo Paymaster. Smart Account execution succeeded.
                        </p>
                        <p className="text-xs font-mono bg-surface-strong p-2 rounded-xl mt-2 text-ink-strong">
                          Tx Hash: {txHash}
                        </p>
                      </div>
                      <button
                        onClick={() => { setActiveTab("overview"); setTransferStep("idle"); }}
                        className="bg-surface-strong hover:bg-surface-strong/80 text-ink-strong px-6 py-2.5 rounded-full font-semibold text-xs"
                      >
                        Back to dashboard
                      </button>
                    </div>
                  )}
                </div>
              )}

              {/* ── SUB-PANEL: POLICY & LIMITS ── */}
              {activeTab === "policy" && (
                <div className="space-y-6">
                  {/* Daily spending limit */}
                  <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-6">
                    <div className="flex justify-between items-center">
                      <h3 className="text-base font-semibold text-ink-strong">Daily spending limits</h3>
                      <Sliders className="w-5 h-5 text-kivo-blue" />
                    </div>

                    <p className="text-sm text-body-gray leading-relaxed">
                      You can adjust your daily transfer cap without triggering a biometric approval flow on every spend. Limit resets every rolling 24 hours.
                    </p>

                    <div className="space-y-4">
                      <div className="flex justify-between items-center">
                        <span className="text-xs font-semibold text-body-gray tracking-wider uppercase">Active Limit</span>
                        <span className="text-lg font-mono font-medium text-ink-strong">${tempLimit.toLocaleString()} USDC</span>
                      </div>
                      
                      <input
                        type="range"
                        min="100"
                        max="10000"
                        step="100"
                        value={tempLimit}
                        onChange={(e) => setTempLimit(parseInt(e.target.value))}
                        className="w-full h-1.5 bg-surface-strong rounded-lg appearance-none cursor-pointer accent-kivo-blue"
                      />

                      <div className="flex justify-between text-[10px] font-bold text-muted-gray uppercase">
                        <span>$100 USDC</span>
                        <span>$10,000 USDC</span>
                      </div>

                      {tempLimit !== dailyLimit && (
                        <button
                          onClick={() => { setDailyLimit(tempLimit); showNotification(`Daily limit updated to $${tempLimit} USDC.`); }}
                          className="w-full h-11 bg-kivo-blue hover:bg-kivo-blue-hover text-white rounded-full font-semibold text-sm transition-all"
                        >
                          Apply New Limit Configuration
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Ephemeral Session Keys list */}
                  <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-6">
                    <div className="flex justify-between items-center">
                      <div>
                        <h3 className="text-base font-semibold text-ink-strong">Active dApp Session Keys</h3>
                        <p className="text-xs text-body-gray mt-1">Allows dApps to interact with your wallet scoped to daily caps and expiry without popup prompts.</p>
                      </div>
                      <Key className="w-5 h-5 text-kivo-blue" />
                    </div>

                    <div className="space-y-3">
                      {sessions.length === 0 ? (
                        <p className="text-xs text-muted-gray py-4 text-center">No active session keys found.</p>
                      ) : (
                        sessions.map((sess) => (
                          <div key={sess.id} className="flex flex-col sm:flex-row justify-between sm:items-center p-4 bg-surface-soft border border-hairline rounded-2xl gap-3">
                            <div className="space-y-1">
                              <p className="text-sm font-semibold text-ink-strong">{sess.name}</p>
                              <div className="flex flex-wrap gap-x-2 gap-y-1 text-[11px] text-body-gray">
                                <span className="font-mono">Target: {sess.contract}</span>
                                <span>•</span>
                                <span>Limit: {sess.limit}</span>
                                <span>•</span>
                                <span>Spent: {sess.spent}</span>
                              </div>
                            </div>
                            <div className="flex items-center justify-between sm:justify-end gap-3 border-t sm:border-t-0 pt-2 sm:pt-0">
                              <span className="text-[10px] font-bold bg-kivo-blue/10 text-kivo-blue px-2.5 py-1 rounded-full uppercase tracking-wider select-none">{sess.expires}</span>
                              <button
                                onClick={() => handleRevokeSession(sess.id, sess.name)}
                                className="w-8 h-8 rounded-full hover:bg-semantic-red/10 text-muted-gray hover:text-semantic-red flex items-center justify-center transition-colors"
                                title="Revoke Session Key"
                              >
                                <Trash2 className="w-4 h-4" />
                              </button>
                            </div>
                          </div>
                        ))
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* ── SUB-PANEL: GUARDIANS ── */}
              {activeTab === "guardians" && (
                <div className="space-y-6">
                  {/* Guardian Overview */}
                  <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-6">
                    <div className="flex justify-between items-center">
                      <div>
                        <h3 className="text-base font-semibold text-ink-strong">Social Recovery System</h3>
                        <p className="text-xs text-body-gray mt-1">If you lose your device or biometrics, guardians can vote to rotate your wallet credentials safely.</p>
                      </div>
                      <Users className="w-5 h-5 text-kivo-blue" />
                    </div>

                    {/* Progress Indicator */}
                    <div className="bg-surface-soft border border-hairline rounded-2xl p-4 flex items-center gap-4">
                      <div className="w-12 h-12 rounded-full bg-kivo-blue/10 flex items-center justify-center text-kivo-blue text-lg font-bold select-none">
                        2/3
                      </div>
                      <div className="flex-1 space-y-1">
                        <p className="text-sm font-semibold text-ink-strong">Recovery Quorum Met</p>
                        <p className="text-xs text-body-gray">At least 2 approvals required to execute rotational updates.</p>
                        <div className="w-full bg-surface-strong h-1.5 rounded-full overflow-hidden">
                          <div className="bg-kivo-blue h-full w-[66.6%]"></div>
                        </div>
                      </div>
                    </div>

                    {/* Active Guardians List */}
                    <div className="space-y-3">
                      <div className="flex justify-between items-center">
                        <span className="text-xs font-semibold text-body-gray tracking-wider uppercase">Active Guardians ({guardians.length + 1})</span>
                        <button
                          onClick={() => setIsAddingGuardian(!isAddingGuardian)}
                          className="inline-flex items-center gap-1 text-xs font-bold text-kivo-blue hover:text-kivo-blue-hover"
                        >
                          <Plus className="w-3.5 h-3.5" /> Add Guardian
                        </button>
                      </div>

                      {/* Add Guardian Form */}
                      {isAddingGuardian && (
                        <form onSubmit={handleAddGuardian} className="p-4 border border-kivo-blue/30 rounded-2xl space-y-4 animate-in fade-in slide-in-from-top-2">
                          <h4 className="text-xs font-bold text-ink-strong uppercase">Add New Guardian</h4>
                          
                          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            <input
                              type="text"
                              required
                              placeholder="Name / Alias"
                              value={newGuardianName}
                              onChange={(e) => setNewGuardianName(e.target.value)}
                              className="h-10 px-3 rounded-lg border border-hairline focus:border-kivo-blue focus:outline-none text-xs bg-white text-ink-strong"
                            />
                            <input
                              type="text"
                              required
                              placeholder="Stellar Public Address"
                              value={newGuardianAddress}
                              onChange={(e) => setNewGuardianAddress(e.target.value)}
                              className="h-10 px-3 rounded-lg border border-hairline focus:border-kivo-blue focus:outline-none text-xs font-mono bg-white text-ink-strong"
                            />
                          </div>

                          <div className="flex justify-end gap-2 pt-2">
                            <button
                              type="button"
                              onClick={() => setIsAddingGuardian(false)}
                              className="h-9 px-4 rounded-full hover:bg-surface-strong text-xs font-semibold text-body-gray"
                            >
                              Cancel
                            </button>
                            <button
                              type="submit"
                              className="h-9 px-4 bg-kivo-blue hover:bg-kivo-blue-hover text-white rounded-full text-xs font-semibold"
                            >
                              Confirm Addition
                            </button>
                          </div>
                        </form>
                      )}

                      {/* Guardian items */}
                      <div className="space-y-2">
                        {/* Device Owner Signer Row */}
                        <div className="flex items-center justify-between p-3.5 bg-surface-soft border border-hairline rounded-2xl">
                          <div className="flex items-center gap-3">
                            <div className="w-8 h-8 rounded-full bg-kivo-blue/10 flex items-center justify-center text-kivo-blue">
                              <Fingerprint className="w-4 h-4" />
                            </div>
                            <div>
                              <p className="text-xs font-semibold text-ink-strong">Owner Biometric Key (Your Phone)</p>
                              <p className="text-[10px] text-body-gray font-mono">Primary sign authority</p>
                            </div>
                          </div>
                          <span className="text-[9px] font-bold bg-kivo-blue/15 text-kivo-blue px-2 py-0.5 rounded-full select-none">PRIMARY OWNER</span>
                        </div>

                        {guardians.map((guardian, index) => (
                          <div key={index} className="flex items-center justify-between p-3.5 bg-white border border-hairline rounded-2xl">
                            <div className="flex items-center gap-3">
                              <div className="w-8 h-8 rounded-full bg-surface-strong flex items-center justify-center text-body-gray">
                                <Users className="w-4 h-4" />
                              </div>
                              <div>
                                <p className="text-xs font-semibold text-ink-strong">{guardian.alias}</p>
                                <p className="text-[10px] text-body-gray font-mono">Address: {guardian.address}</p>
                              </div>
                            </div>
                            <div className="flex items-center gap-2">
                              <span className="text-[9px] text-muted-gray">Added: {guardian.addedAt}</span>
                              <button
                                onClick={() => {
                                  setGuardians((prev) => prev.filter((_, idx) => idx !== index));
                                  showNotification(`Guardian ${guardian.alias} removed.`);
                                }}
                                className="w-7 h-7 rounded-full hover:bg-semantic-red/10 text-muted-gray hover:text-semantic-red flex items-center justify-center transition-colors"
                              >
                                <X className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Right Column: Sidebar / System Specs info card */}
            <div className="lg:col-span-4 space-y-6">
              {/* Wallet Contract Info Card */}
              <div className="bg-white rounded-3xl p-6 border border-hairline shadow-sm space-y-4">
                <h4 className="text-xs font-bold text-body-gray tracking-wider uppercase">Wallet Specs & Nodes</h4>
                
                <div className="space-y-3">
                  <div className="flex justify-between items-center text-xs pb-2 border-b border-hairline">
                    <span className="text-body-gray">Active Contract</span>
                    <span className="font-mono text-ink-strong font-semibold">CA3W...KIVO</span>
                  </div>
                  <div className="flex justify-between items-center text-xs pb-2 border-b border-hairline">
                    <span className="text-body-gray">Smart Account ABI</span>
                    <span className="text-ink-strong font-medium flex items-center gap-1 cursor-pointer hover:text-kivo-blue">
                      v1.0.0-Soroban <ExternalLink className="w-3 h-3" />
                    </span>
                  </div>
                  <div className="flex justify-between items-center text-xs pb-2 border-b border-hairline">
                    <span className="text-body-gray">Protocol Version</span>
                    <span className="text-ink-strong font-mono">Stellar Protocol 21</span>
                  </div>
                  <div className="flex justify-between items-center text-xs">
                    <span className="text-body-gray">Replay Protection</span>
                    <span className="text-semantic-green bg-semantic-green/10 px-2.5 py-0.5 rounded-full font-bold select-none text-[10px]">ACTIVE</span>
                  </div>
                </div>
              </div>

              {/* Developer Testing Console Card */}
              <div className="bg-surface-dark rounded-3xl p-6 text-white border border-surface-dark-elevated shadow-xl space-y-4">
                <h4 className="text-xs font-bold text-white/50 tracking-wider uppercase flex items-center gap-2">
                  <Cpu className="w-4 h-4 text-kivo-blue" />
                  <span>Developer testing console</span>
                </h4>
                
                <p className="text-[11px] text-white/60 leading-relaxed font-light">
                  This console allows developers to trigger smart contract testing hooks to evaluate the Soroban Custom Auth workflow.
                </p>

                <div className="space-y-2 pt-2">
                  <button
                    onClick={() => showNotification("Simulated: Soroban persistent TTL bumped for all signer entries.")}
                    className="w-full text-left bg-white/5 hover:bg-white/10 text-white/80 hover:text-white p-3 rounded-xl border border-white/5 text-xs font-medium flex items-center justify-between transition-colors"
                  >
                    <span>Trigger storage extend_ttl()</span>
                    <ChevronRight className="w-3.5 h-3.5" />
                  </button>

                  <button
                    onClick={() => {
                      const mockTarget = "CA" + Array.from({ length: 10 }, () => "ABCDEF0123456789"[Math.floor(Math.random() * 16)]).join("");
                      const newSess: SessionKeyItem = {
                        id: `sess_${Date.now()}`,
                        name: "Mock Integration",
                        contract: `${mockTarget}...DEMO`,
                        limit: "250 USDC",
                        spent: "0 USDC",
                        expires: "2h left",
                      };
                      setSessions((prev) => [...prev, newSess]);
                      showNotification("Created a mock temporary session key via CLI.");
                    }}
                    className="w-full text-left bg-white/5 hover:bg-white/10 text-white/80 hover:text-white p-3 rounded-xl border border-white/5 text-xs font-medium flex items-center justify-between transition-colors"
                  >
                    <span>Deploy temporary session key</span>
                    <ChevronRight className="w-3.5 h-3.5" />
                  </button>

                  <button
                    onClick={() => showNotification("CLI Recovery Triggered: Recovery proposal #104 has been started in recovery-module.")}
                    className="w-full text-left bg-white/5 hover:bg-white/10 text-white/80 hover:text-white p-3 rounded-xl border border-white/5 text-xs font-medium flex items-center justify-between transition-colors"
                  >
                    <span>Trigger mock recovery proposal</span>
                    <ChevronRight className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            </div>
          </main>
        </div>
      )}
    </div>
  );
}
