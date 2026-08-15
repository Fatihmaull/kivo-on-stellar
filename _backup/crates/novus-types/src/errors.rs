use soroban_sdk::contracterror;

/// Comprehensive error type for all NovusWallet contracts.
/// Each error code is unique across the entire system to simplify debugging.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum WalletError {
    // ── Initialization (1xx) ──
    NotInitialized = 100,
    AlreadyInitialized = 101,

    // ── Signature & Auth (2xx) ──
    InvalidSignature = 200,
    InvalidNonce = 201,
    SignerNotFound = 202,
    SignerExpired = 203,
    SignerAlreadyExists = 204,
    ReplayDetected = 205,
    InvalidAuthContext = 206,

    // ── Policy (3xx) ──
    PolicyViolation = 300,
    SpendingLimitExceeded = 301,
    UnauthorizedTarget = 302,
    UnauthorizedFunction = 303,
    AmountExceedsSessionCap = 304,

    // ── Recovery (4xx) ──
    RecoveryNotReady = 400,
    TimelockActive = 401,
    InsufficientGuardians = 402,
    DuplicateGuardian = 403,
    GuardianNotFound = 404,
    ProposalNotFound = 405,
    ProposalAlreadyExecuted = 406,
    ProposalCancelled = 407,
    CooldownActive = 408,

    // ── Session Keys (5xx) ──
    SessionKeyInvalid = 500,
    SessionKeyExpired = 501,
    SessionTotalExceeded = 502,

    // ── Paymaster / Fees (6xx) ──
    UnsupportedFeeToken = 600,
    InsufficientFeeBalance = 601,
    ConversionFailed = 602,

    // ── General (9xx) ──
    Unauthorized = 900,
    InternalError = 999,
}
