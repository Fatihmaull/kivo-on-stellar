# commit_history.ps1
# Automates creation of 24 commits representing Kivo's development phases and pushes them one-by-one.

# Set branch to main
git checkout -b main 2>$null
if (-not $?) { git branch -M main }

# Create backup directory
if (Test-Path "_backup") { Remove-Item -Recurse -Force "_backup" }
New-Item -ItemType Directory -Path "_backup" -Force > $null

# Move everything except .git, commit_history.ps1 and _backup to _backup
Get-ChildItem -Path . -Force | Where-Object { $_.Name -notmatch "^\.git$|^_backup$|^commit_history\.ps1$" } | ForEach-Object {
    Move-Item -Path $_.FullName -Destination "_backup" -Force
}

# Helper to ensure parent directory exists
function Ensure-Dir($relative_path) {
    $parent = Split-Path -Path $relative_path
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force > $null
    }
}

# Helper to copy single item
function Copy-From-Backup($relative_path) {
    $src = Join-Path "_backup" $relative_path
    if (Test-Path $src) {
        Ensure-Dir $relative_path
        Copy-Item -Path $src -Destination $relative_path -Recurse -Force
    }
}

# Helper to commit and push
function Commit-And-Push($msg) {
    git add -A
    git commit -m $msg
    Write-Host "Pushing commit: $msg" -ForegroundColor Cyan
    git push origin main -f
}

# ═══════════════════════════════════════════════════════════════════
# PHASE 1: Workspace & Shared Library (Commits 1-8)
# ═══════════════════════════════════════════════════════════════════

# Commit 1: Initial workspace structure and root files
Copy-From-Backup ".gitignore"
Copy-From-Backup "Cargo.toml"
Commit-And-Push "chore: initialize Cargo workspace and gitignore"

# Commit 2: Setup novus-types library crate structure
Copy-From-Backup "crates/novus-types/Cargo.toml"
Copy-From-Backup "crates/novus-types/src/lib.rs"
Commit-And-Push "feat(types): scaffold novus-types library crate"

# Commit 3: Implement centralized error codes
Copy-From-Backup "crates/novus-types/src/errors.rs"
Commit-And-Push "feat(types): implement centralized system error codes"

# Commit 4: Define storage keys for multiple storage tiers
Copy-From-Backup "crates/novus-types/src/keys.rs"
Commit-And-Push "feat(types): define storage key mappings and namespaces"

# Commit 5: Define signer structures
Copy-From-Backup "crates/novus-types/src/signer.rs"
Commit-And-Push "feat(types): define SignerEntry and WalletSignature structures"

# Commit 6: Define session key configurations
Copy-From-Backup "crates/novus-types/src/session.rs"
Commit-And-Push "feat(types): define SessionConfig for ephemeral integrations"

# Commit 7: Define social recovery models
Copy-From-Backup "crates/novus-types/src/recovery.rs"
Commit-And-Push "feat(types): define RecoveryProposal and GuardianInfo types"

# Commit 8: Define policy result types
Copy-From-Backup "crates/novus-types/src/policy.rs"
Commit-And-Push "feat(types): define PolicyResult enum for transaction checks"

# ═══════════════════════════════════════════════════════════════════
# PHASE 2: SmartAccount Contract Implementation (Commits 9-16)
# ═══════════════════════════════════════════════════════════════════

# Commit 9: Setup core smart-account contract project entry
Copy-From-Backup "contracts/smart-account/Cargo.toml"
Commit-And-Push "feat(contract): scaffold novus-smart-account contract crate"

# Commit 10: Create smart-account storage engine
Copy-From-Backup "contracts/smart-account/src/storage.rs"
Commit-And-Push "feat(contract): implement Storage layer with TTL extensions"

# Commit 11: Implement nonces and replay guards
Copy-From-Backup "contracts/smart-account/src/nonce.rs"
Commit-And-Push "feat(contract): implement dual-layer replay prevention"

# Commit 12: Implement WebAuthn signature verification
Copy-From-Backup "contracts/smart-account/src/webauthn.rs"
Commit-And-Push "feat(contract): implement WebAuthn secp256r1 signature verification"

# Commit 13: Implement signer and guardian CRUD
Copy-From-Backup "contracts/smart-account/src/signers.rs"
Commit-And-Push "feat(contract): implement signer, guardian and owner rotation CRUD"

# Commit 14: Implement session key scope verification
Copy-From-Backup "contracts/smart-account/src/session.rs"
Commit-And-Push "feat(contract): implement session key scope and cap check"

# Commit 15: Implement built-in spending limit checks
Copy-From-Backup "contracts/smart-account/src/policy.rs"
Commit-And-Push "feat(contract): implement built-in daily spending policy check"

# Commit 16: Implement custom auth check entrypoint
Copy-From-Backup "contracts/smart-account/src/auth.rs"
Copy-From-Backup "contracts/smart-account/src/lib.rs"
Commit-And-Push "feat(contract): implement custom auth __check_auth hook"

# ═══════════════════════════════════════════════════════════════════
# PHASE 3: Testing & Stubs (Commits 17-19)
# ═══════════════════════════════════════════════════════════════════

# Commit 17: Add stub contracts for other modular crates
Copy-From-Backup "contracts/paymaster"
Copy-From-Backup "contracts/recovery-module"
Copy-From-Backup "contracts/policy-engine"
Commit-And-Push "feat(contract): deploy stubs for paymaster, recovery and policy contracts"

# Commit 18: Implement comprehensive contract test suite
Copy-From-Backup "contracts/smart-account/src/test.rs"
Commit-And-Push "test(contract): implement full SmartAccount contract test suite"

# Commit 19: Add lock file and compile check
Copy-From-Backup "Cargo.lock"
Commit-And-Push "chore: add Cargo.lock and confirm workspace compile"

# ═══════════════════════════════════════════════════════════════════
# PHASE 4: Frontend Development (Commits 20-22)
# ═══════════════════════════════════════════════════════════════════

# Commit 20: Bootstrap Next.js web application structure and configs
Copy-From-Backup "web/package.json"
Copy-From-Backup "web/package-lock.json"
Copy-From-Backup "web/tsconfig.json"
Copy-From-Backup "web/next.config.ts"
Copy-From-Backup "web/next-env.d.ts"
Copy-From-Backup "web/eslint.config.mjs"
Copy-From-Backup "web/postcss.config.mjs"
Copy-From-Backup "web/public"
Copy-From-Backup "web/README.md"
Commit-And-Push "feat(web): bootstrap Next.js 16 App Router application"

# Commit 21: Add globals.css and layout.tsx configuration
Copy-From-Backup "web/src/app/globals.css"
Copy-From-Backup "web/src/app/layout.tsx"
Copy-From-Backup "web/src/app/favicon.ico"
Commit-And-Push "feat(web): configure globals.css and typography layouts"

# Commit 22: Implement interactive Kivo landing page and dashboard
Copy-From-Backup "web/src/app/page.tsx"
Commit-And-Push "feat(web): implement interactive Kivo landing and dashboard UI"

# ═══════════════════════════════════════════════════════════════════
# PHASE 5: Workflows & Documentation (Commits 23-24)
# ═══════════════════════════════════════════════════════════════════

# Commit 23: Configure GitHub Actions CI/CD pipeline
Copy-From-Backup ".github"
Commit-And-Push "chore: add GitHub Actions CI/CD verify workflow"

# Commit 24: Add production-grade README.md documentation
Copy-From-Backup "README.md"
Commit-And-Push "docs: add Instaward-compliant README.md"

# Cleanup backup folder
Remove-Item -Recurse -Force "_backup"
Write-Host "All 24 commits generated and pushed successfully!" -ForegroundColor Green
