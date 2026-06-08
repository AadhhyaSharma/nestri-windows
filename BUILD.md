# Nestri Windows — Build Instructions

## Prerequisites

Install these on your Windows build machine:

### 1. Rust (MSVC toolchain)
```
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
```

### 2. Visual Studio Build Tools (C++ workload)
```
winget install Microsoft.VisualStudio.2022.BuildTools
```
Select: **Desktop development with C++**

### 3. GStreamer 1.24 (Runtime + Development)
Download both MSIs from https://gstreamer.freedesktop.org/download/
- `gstreamer-1.0-msvc-x86_64-1.24.0.msi`  (runtime)
- `gstreamer-1.0-devel-msvc-x86_64-1.24.0.msi` (dev headers)

Install both. Default path: `C:\gstreamer\1.0\msvc_x86_64`

Set environment variables:
```powershell
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = "C:\gstreamer\1.0\msvc_x86_64"
$env:PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\bin;$env:PATH"
$env:PKG_CONFIG_PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\lib\pkgconfig"
```

### 4. pkg-config for Windows
```
winget install bloodrock.pkg-config-lite
```
Or install via MSYS2: `pacman -S pkgconf`

### 5. NSIS (for building the installer)
```
winget install NSIS.NSIS
```

### 6. Go (for the relay binary)
```
winget install GoLang.Go
```

---

## Build Steps

### Step 1 — Build the Nestri server
```powershell
cd nestri-windows
cargo build --release --target x86_64-pc-windows-msvc
```
Output: `target\x86_64-pc-windows-msvc\release\nestri-server.exe`

### Step 2 — Build the launcher
```powershell
rustc installer\nestri-launcher.rs ^
  --edition 2021 ^
  --target x86_64-pc-windows-msvc ^
  -C opt-level=3 ^
  -L "C:\gstreamer\1.0\msvc_x86_64\lib" ^
  -o installer\bin\nestri-launcher.exe
```

### Step 3 — Build the relay (Go → Windows)
```powershell
cd ..\packages\relay  # original Nestri relay source
set GOOS=windows
set GOARCH=amd64
go build -ldflags="-s -w" -o ..\..\nestri-windows\installer\bin\nestri-relay.exe
```

### Step 4 — Gather installer dependencies
Create `installer\deps\` and place:
- `gstreamer-1.0-msvc-x86_64-1.24.0.msi` — from GStreamer download page
- `ViGEmBus_Setup_x64.exe` — from https://github.com/nefarius/ViGEmBus/releases

Copy binaries:
```powershell
mkdir installer\bin
copy target\x86_64-pc-windows-msvc\release\nestri-server.exe installer\bin\
# relay and launcher already built to installer\bin\
```

### Step 5 — Build the installer EXE
```powershell
cd installer
makensis nestri-installer.nsi
```
Output: `installer\NestriSetup.exe` — the single 1-click installer

---

## Quick Build Script (PowerShell)

Save as `build-all.ps1` and run from the `nestri-windows` folder:

```powershell
# Set GStreamer paths
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = "C:\gstreamer\1.0\msvc_x86_64"
$env:PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\bin;$env:PATH"
$env:PKG_CONFIG_PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\lib\pkgconfig"

# 1. Build server
Write-Host "[1/4] Building nestri-server..." -ForegroundColor Cyan
cargo build --release --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { Write-Error "Server build failed"; exit 1 }

# 2. Build launcher
Write-Host "[2/4] Building nestri-launcher..." -ForegroundColor Cyan
rustc installer\nestri-launcher.rs --edition 2021 --target x86_64-pc-windows-msvc -C opt-level=3 -o installer\bin\nestri-launcher.exe
if ($LASTEXITCODE -ne 0) { Write-Error "Launcher build failed"; exit 1 }

# 3. Build relay
Write-Host "[3/4] Building nestri-relay (Go)..." -ForegroundColor Cyan
Push-Location ..\packages\relay
$env:GOOS    = "windows"
$env:GOARCH  = "amd64"
go build -ldflags="-s -w" -o ..\..\nestri-windows\installer\bin\nestri-relay.exe
Pop-Location
if ($LASTEXITCODE -ne 0) { Write-Error "Relay build failed"; exit 1 }

# 4. Copy server binary and build installer
Write-Host "[4/4] Building installer..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path installer\bin | Out-Null
Copy-Item target\x86_64-pc-windows-msvc\release\nestri-server.exe installer\bin\
Push-Location installer
makensis nestri-installer.nsi
Pop-Location
if ($LASTEXITCODE -ne 0) { Write-Error "Installer build failed"; exit 1 }

Write-Host ""
Write-Host "BUILD COMPLETE!" -ForegroundColor Green
Write-Host "Output: installer\NestriSetup.exe" -ForegroundColor Yellow
```

---

## What Each Binary Does

| Binary | Description |
|---|---|
| `nestri-server.exe` | Main Rust streaming server — DXGI GPU detection, D3D11 screen capture, NVENC encoding, WebRTC via GStreamer |
| `nestri-relay.exe` | Go-compiled relay — WebRTC SFU, forwards stream to browser clients. Identical to original Linux relay. |
| `nestri-launcher.exe` | Small Windows GUI — system tray icon, starts/stops server+relay, reads nestri.env config |
| `NestriSetup.exe` | NSIS installer — bundles all of the above + GStreamer runtime + ViGEmBus driver |

---

## Runtime Requirements (on the host machine)

The installer handles these automatically, but if running manually:
- Windows 10/11 64-bit
- NVIDIA RTX GPU with latest drivers (for NVENC)
- GStreamer 1.24 runtime (installed by NestriSetup.exe)
- ViGEmBus driver (installed by NestriSetup.exe, for controller input)
- No Docker, no WSL, no Linux layer — pure Windows.

---

## GStreamer Plugins Required

The server needs these GStreamer plugins (included in the MSVC full install):
- `d3d11` — D3D11 screen capture (`d3d11screencapturesrc`)
- `wasapi` — Windows audio (`wasapisrc`)
- `nvcodec` — NVIDIA NVENC (`nvh264enc`, `nvh265enc`)
- `webrtc` — WebRTC sink
- `opus` — Audio encoding
- `rtp` — RTP packetization

Install GStreamer with **Complete** installation to get all plugins.
