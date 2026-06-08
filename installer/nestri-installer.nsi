; Nestri Windows Installer
; NSIS script — builds a single-click Setup.exe
; Bundles: GStreamer runtime, nestri-server.exe, nestri-relay.exe, ViGEmBus installer

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; ── Installer metadata ────────────────────────────────────────────────────────
Name              "Nestri Windows Server"
OutFile           "NestriSetup.exe"
InstallDir        "$PROGRAMFILES64\Nestri"
InstallDirRegKey  HKLM "Software\Nestri" "InstallDir"
RequestExecutionLevel admin
SetCompressor     /SOLID lzma

; ── MUI Settings ──────────────────────────────────────────────────────────────
!define MUI_ABORTWARNING
!define MUI_ICON   "assets\nestri.ico"
!define MUI_UNICON "assets\nestri.ico"
!define MUI_WELCOMEPAGE_TITLE  "Nestri Windows Server Setup"
!define MUI_WELCOMEPAGE_TEXT   "This will install the Nestri streaming server on your PC.$\r$\n$\r$\nNestri lets you stream your Windows desktop to any browser, with GPU-accelerated encoding via your NVIDIA RTX card.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_RUN     "$INSTDIR\nestri-launcher.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Nestri Server"

; ── Pages ─────────────────────────────────────────────────────────────────────
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE    "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
Page custom RelayConfigPage RelayConfigPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; ── Custom config page variables ──────────────────────────────────────────────
Var RelayURL
Var RoomName
Var Dialog
Var LabelRelay
Var FieldRelay
Var LabelRoom
Var FieldRoom

Function RelayConfigPage
  !insertmacro MUI_HEADER_TEXT "Nestri Configuration" "Set your relay server and stream room."
  nsDialogs::Create 1018
  Pop $Dialog
  ${If} $Dialog == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 12u "Relay URL (libp2p multiaddr):"
  Pop $LabelRelay
  ${NSD_CreateText} 0 13u 100% 12u $RelayURL
  Pop $FieldRelay

  ${NSD_CreateLabel} 0 35u 100% 12u "Stream Room Name:"
  Pop $LabelRoom
  ${NSD_CreateText} 0 48u 100% 12u $RoomName
  Pop $FieldRoom

  nsDialogs::Show
FunctionEnd

Function RelayConfigPageLeave
  ${NSD_GetText} $FieldRelay $RelayURL
  ${NSD_GetText} $FieldRoom  $RoomName
FunctionEnd

; ── Installation sections ─────────────────────────────────────────────────────
Section "GStreamer Runtime" SecGStreamer
  SectionIn RO  ; Required

  SetOutPath "$INSTDIR\gstreamer"
  DetailPrint "Installing GStreamer 1.24 runtime..."

  ; Bundle the GStreamer MSI and install silently
  File "deps\gstreamer-1.0-msvc-x86_64-1.24.0.msi"
  ExecWait 'msiexec /i "$INSTDIR\gstreamer\gstreamer-1.0-msvc-x86_64-1.24.0.msi" /qn TARGETDIR="$INSTDIR\gstreamer\runtime"' $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONEXCLAMATION "GStreamer installation failed (code $0). Please install manually from https://gstreamer.freedesktop.org/download/"
  ${EndIf}
  Delete "$INSTDIR\gstreamer\gstreamer-1.0-msvc-x86_64-1.24.0.msi"

  ; Add GStreamer bin to system PATH for this install
  EnVar::AddValue "PATH" "$INSTDIR\gstreamer\runtime\1.0\msvc_x86_64\bin"
SectionEnd

Section "ViGEmBus Driver" SecViGEm
  SetOutPath "$INSTDIR\vigem"
  DetailPrint "Installing ViGEmBus virtual gamepad driver..."
  File "deps\ViGEmBus_Setup_x64.exe"
  ExecWait '"$INSTDIR\vigem\ViGEmBus_Setup_x64.exe" /quiet' $0
  ${If} $0 != 0
    DetailPrint "ViGEmBus install returned $0 — controller input may not work."
  ${EndIf}
  Delete "$INSTDIR\vigem\ViGEmBus_Setup_x64.exe"
SectionEnd

Section "Nestri Server" SecMain
  SectionIn RO

  SetOutPath "$INSTDIR"
  DetailPrint "Installing Nestri server binaries..."

  File "bin\nestri-server.exe"
  File "bin\nestri-relay.exe"
  File "bin\nestri-launcher.exe"

  ; Write config file with user-provided relay URL and room name
  FileOpen  $9 "$INSTDIR\nestri.env" w
  FileWrite $9 "NESTRI_RELAY_URL=$RelayURL$\r$\n"
  FileWrite $9 "NESTRI_ROOM=$RoomName$\r$\n"
  FileWrite $9 "NESTRI_GPU_VENDOR=nvidia$\r$\n"
  FileWrite $9 "NESTRI_VIDEO_CODEC=h264$\r$\n"
  FileWrite $9 "NESTRI_ENCODER_TYPE=hardware$\r$\n"
  FileWrite $9 "NESTRI_LATENCY=lowest-latency$\r$\n"
  FileWrite $9 "NESTRI_RATE_CONTROL=cbr:8000$\r$\n"
  FileWrite $9 "NESTRI_FRAMERATE=60$\r$\n"
  FileWrite $9 "NESTRI_MONITOR=0$\r$\n"
  FileWrite $9 "NESTRI_AUDIO_BITRATE=128$\r$\n"
  FileClose $9

  ; Write readme
  File "README.txt"
SectionEnd

Section "Start Menu Shortcuts" SecShortcuts
  CreateDirectory "$SMPROGRAMS\Nestri"
  CreateShortCut  "$SMPROGRAMS\Nestri\Nestri Server.lnk"  "$INSTDIR\nestri-launcher.exe"
  CreateShortCut  "$SMPROGRAMS\Nestri\Uninstall Nestri.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortCut  "$DESKTOP\Nestri Server.lnk"             "$INSTDIR\nestri-launcher.exe"
SectionEnd

Section "Register Installation"
  WriteRegStr   HKLM "Software\Nestri" "InstallDir" "$INSTDIR"
  WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "DisplayName"     "Nestri Windows Server"
  WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "DisplayVersion"  "0.1.0-windows"
  WriteRegStr   HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "Publisher"       "Nestri Windows Port"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri" \
                "NoRepair" 1
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

; ── Uninstaller ───────────────────────────────────────────────────────────────
Section "Uninstall"
  Delete "$INSTDIR\nestri-server.exe"
  Delete "$INSTDIR\nestri-relay.exe"
  Delete "$INSTDIR\nestri-launcher.exe"
  Delete "$INSTDIR\nestri.env"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir  /r "$INSTDIR\gstreamer"
  RMDir  "$INSTDIR"

  Delete "$SMPROGRAMS\Nestri\Nestri Server.lnk"
  Delete "$SMPROGRAMS\Nestri\Uninstall Nestri.lnk"
  Delete "$DESKTOP\Nestri Server.lnk"
  RMDir  "$SMPROGRAMS\Nestri"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nestri"
  DeleteRegKey HKLM "Software\Nestri"

  EnVar::DeleteValue "PATH" "$INSTDIR\gstreamer\runtime\1.0\msvc_x86_64\bin"
SectionEnd

; ── Section descriptions ──────────────────────────────────────────────────────
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecGStreamer} "GStreamer 1.24 multimedia framework (required for streaming)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecViGEm}    "ViGEmBus virtual gamepad driver (required for controller passthrough)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain}     "Nestri server and relay binaries"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecShortcuts} "Desktop and Start Menu shortcuts"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInit
  StrCpy $RelayURL ""
  StrCpy $RoomName "nestri-windows"
FunctionEnd
