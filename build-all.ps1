# Nestri Windows — Full Build Script
# Run this from the nestri-windows\ directory on your Windows machine.
# Requires: Rust (MSVC), Go, GStreamer 1.24 dev, NSIS, pkg-config
# See BUILD.md for full prerequisites.

param(
    [string]$GStreamerRoot = "C:\gstreamer\1.0\msvc_x86_64",
    [string]$NestriRelaySrc = "..\packages\relay",
    [switch]$SkipRelay,
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"

function Step($n, $total, $msg) {
    Write-Host ""
    Write-Host "[$n/$total] $msg" -ForegroundColor Cyan
    Write-Host ("-" * 60) -ForegroundColor DarkGray
}

function Ok($msg) { Write-Host "  OK: $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "  FAIL: $msg" -ForegroundColor Red; exit 1 }

# ── Validate environment ──────────────────────────────────────────────────────
Write-Host ""
Write-Host "╔══════════════════════════════════════════════╗" -ForegroundColor Yellow
Write-Host "║   Nestri Windows Build Script                ║" -ForegroundColor Yellow
Write-Host "╚══════════════════════════════════════════════╝" -ForegroundColor Yellow
Write-Host ""

# Check GStreamer
if (-not (Test-Path $GStreamerRoot)) {
    Write-Host "GStreamer not found at $GStreamerRoot" -ForegroundColor Red
    Write-Host "Download from: https://gstreamer.freedesktop.org/download/" -ForegroundColor Yellow
    Write-Host "Then re-run: .\build-all.ps1 -GStreamerRoot 'C:\path\to\gstreamer\1.0\msvc_x86_64'"
    exit 1
}
Ok "GStreamer found at $GStreamerRoot"

# Set GStreamer env
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $GStreamerRoot
$env:PATH            = "$GStreamerRoot\bin;$env:PATH"
$env:PKG_CONFIG_PATH = "$GStreamerRoot\lib\pkgconfig"

# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Fail "Rust/cargo not found. Install from https://rustup.rs/" }
Ok "Rust: $(rustc --version)"

# Check Go
if (-not $SkipRelay) {
    if (-not (Get-Command go -ErrorAction SilentlyContinue)) { Fail "Go not found. Install from https://go.dev/dl/" }
    Ok "Go: $(go version)"
}

# Check NSIS
if (-not $SkipInstaller) {
    if (-not (Get-Command makensis -ErrorAction SilentlyContinue)) {
        Write-Host "  WARN: NSIS not found — installer step will be skipped." -ForegroundColor Yellow
        $SkipInstaller = $true
    } else {
        Ok "NSIS found"
    }
}

# Create output directories
New-Item -ItemType Directory -Force -Path "installer\bin"  | Out-Null
New-Item -ItemType Directory -Force -Path "installer\deps" | Out-Null

# ── Step 1: Build nestri-server ───────────────────────────────────────────────
Step 1 4 "Building nestri-server.exe (Rust/MSVC)"

cargo build --release --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { Fail "cargo build failed" }

Copy-Item "target\x86_64-pc-windows-msvc\release\nestri-server.exe" "installer\bin\"
Ok "nestri-server.exe built and copied"

# ── Step 2: Build nestri-launcher ────────────────────────────────────────────
Step 2 4 "Building nestri-launcher.exe (Rust/MSVC)"

$launcherFlags = @(
    "installer\nestri-launcher.rs",
    "--edition", "2021",
    "--target",  "x86_64-pc-windows-msvc",
    "-C", "opt-level=3",
    "-C", "panic=abort",
    "-o", "installer\bin\nestri-launcher.exe"
)
rustc @launcherFlags
if ($LASTEXITCODE -ne 0) { Fail "nestri-launcher build failed" }
Ok "nestri-launcher.exe built"

# ── Step 3: Build nestri-relay (Go) ──────────────────────────────────────────
if (-not $SkipRelay) {
    Step 3 4 "Building nestri-relay.exe (Go/Windows)"

    if (-not (Test-Path $NestriRelaySrc)) {
        Write-Host "  WARN: Relay source not found at $NestriRelaySrc" -ForegroundColor Yellow
        Write-Host "  Clone the full Nestri repo and set -NestriRelaySrc to packages/relay"
        Write-Host "  Skipping relay build..."
    } else {
        Push-Location $NestriRelaySrc
        $env:GOOS   = "windows"
        $env:GOARCH = "amd64"
        $env:CGO_ENABLED = "0"
        go build -ldflags="-s -w" -o "..\..\nestri-windows\installer\bin\nestri-relay.exe" .
        if ($LASTEXITCODE -ne 0) { Pop-Location; Fail "Go relay build failed" }
        Pop-Location
        Ok "nestri-relay.exe built"
        Remove-Item Env:GOOS
        Remove-Item Env:GOARCH
    }
} else {
    Write-Host "[3/4] Skipping relay build (-SkipRelay)" -ForegroundColor DarkGray
}

# ── Step 4: Build installer ───────────────────────────────────────────────────
if (-not $SkipInstaller) {
    Step 4 4 "Building NestriSetup.exe (NSIS)"

    # Check that required deps exist
    $gstMsi = "installer\deps\gstreamer-1.0-msvc-x86_64-1.24.0.msi"
    $vigemExe = "installer\deps\ViGEmBus_Setup_x64.exe"

    $missingDeps = @()
    if (-not (Test-Path $gstMsi))   { $missingDeps += "installer\deps\gstreamer-1.0-msvc-x86_64-1.24.0.msi" }
    if (-not (Test-Path $vigemExe)) { $missingDeps += "installer\deps\ViGEmBus_Setup_x64.exe" }

    if ($missingDeps.Count -gt 0) {
        Write-Host ""
        Write-Host "  Missing installer dependencies:" -ForegroundColor Yellow
        foreach ($d in $missingDeps) {
            Write-Host "    - $d" -ForegroundColor Yellow
        }
        Write-Host ""
        Write-Host "  Download them:" -ForegroundColor Cyan
        Write-Host "    GStreamer MSI: https://gstreamer.freedesktop.org/download/"
        Write-Host "    ViGEmBus:      https://github.com/nefarius/ViGEmBus/releases"
        Write-Host ""
        Write-Host "  Then place them in installer\deps\ and re-run with:"
        Write-Host "  .\build-all.ps1 -SkipRelay"
        Write-Host ""
        Write-Host "  Skipping installer packaging..." -ForegroundColor Yellow
    } else {
        Push-Location installer
        makensis nestri-installer.nsi
        if ($LASTEXITCODE -ne 0) { Pop-Location; Fail "NSIS build failed" }
        Pop-Location
        Ok "NestriSetup.exe built at installer\NestriSetup.exe"
    }
} else {
    Write-Host "[4/4] Skipping installer build (-SkipInstaller)" -ForegroundColor DarkGray
}

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "╔══════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║   BUILD COMPLETE!                            ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "  Binaries in: installer\bin\" -ForegroundColor White
if (Test-Path "installer\NestriSetup.exe") {
    Write-Host "  Installer:   installer\NestriSetup.exe" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  Run NestriSetup.exe on any Windows 10/11 machine with an NVIDIA RTX GPU." -ForegroundColor Cyan
}
Write-Host ""
