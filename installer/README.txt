Nestri Windows Server
=====================
Version: 0.1.0-windows
A pure Windows port of the Nestri cloud gaming/streaming server.

WHAT THIS IS
------------
Nestri lets you stream your Windows desktop (games, apps, or full screen)
to any browser on any device — like your own personal GeForce Now.

This is a complete Windows rewrite of the original Linux-only Nestri server.
No Docker, no WSL, no Linux. Pure Windows + DirectX + NVENC.

HOW TO START
------------
Nestri starts automatically when you finish the installer.
You'll see a Nestri icon in your system tray (bottom-right).

Right-click the tray icon to:
  - Start / Stop the stream server
  - Open the log folder
  - Exit

CONFIGURATION
-------------
Edit: C:\Program Files\Nestri\nestri.env

  NESTRI_RELAY_URL   — Your relay server URL (libp2p multiaddr)
  NESTRI_ROOM        — Stream room name (viewers connect to this)
  NESTRI_FRAMERATE   — Target FPS (default: 60)
  NESTRI_MONITOR     — Monitor to stream (0 = primary)
  NESTRI_RATE_CONTROL — cbr:8000 = 8Mbps CBR (change as needed)

After editing, restart Nestri from the tray icon.

REQUIREMENTS
------------
- Windows 10/11 (64-bit)
- NVIDIA RTX GPU (for hardware NVENC encoding)
- The installer automatically sets up GStreamer and ViGEmBus

CONTROLLER INPUT
----------------
ViGEmBus is installed automatically. It creates a virtual Xbox 360
controller that browser clients can use to send gamepad input to your PC.

TROUBLESHOOTING
---------------
- Black screen: Make sure GStreamer d3d11 plugin is installed (use Complete install)
- No NVENC: Update your NVIDIA driver to the latest version
- Controller not working: Check ViGEmBus is installed (Device Manager → System devices)
- Can't connect: Check your relay URL in nestri.env

OPEN SOURCE
-----------
This is a Windows port of the open-source Nestri project:
https://github.com/nestrilabs/nestri

Windows port source: all files in the nestri-windows/ directory.
