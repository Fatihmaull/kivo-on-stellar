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
    /// The base64url(signature_payload) challenge was not found at the
    /// expected offset in clientDataJSON — the assertion does not cover
    /// this transaction.
    ChallengeMismatch = 210,
    /// clientDataJSON does not contain `"type":"webauthn.get"`.
    WrongCeremonyType = 211,
    /// authenticator_data's rpIdHash does not match the wallet's configured
    /// Relying Party.
    RpIdMismatch = 212,
    /// The User Present (UP) flag was not set in authenticator_data.
    UserPresenceMissing = 213,
    /// The User Verified (UV) flag was not set in authenticator_data
    /// (biometric/PIN gate did not run).
    UserVerificationMissing = 214,
    /// clientDataJSON is missing required fields or is malformed.
    MalformedClientData = 215,
    /// authenticator_data is shorter than the minimum 37 bytes.
    MalformedAssertion = 216,
    /// The signer was registered under a credential epoch that predates
    /// the wallet's current epoch (invalidated by a social recovery).
    StaleCredentialEpoch = 217,
    /// This signer's weight is below what the attempted action requires
    /// (e.g. a session key attempting an owner-only action).
    InsufficientWeight = 218,

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
    /// Removing this guardian would drop the guardian count below the
    /// configured recovery threshold, making recovery permanently
    /// impossible.
    BelowRecoveryThreshold = 409,

    // ── Session Keys (5xx) ──
    SessionKeyInvalid = 500,
    SessionKeyExpired = 501,
    SessionTotalExceeded = 502,

    // ── Paymaster / Fees (6xx) ──
    UnsupportedFeeToken = 600,
    InsufficientFeeBalance = 601,
    ConversionFailed = 602,
    /// The relayer's quoted fee exceeds the caller-signed maximum.
    FeeExceedsMax = 603,

    // ── General (9xx) ──
    Unauthorized = 900,
    InternalError = 999,
}
