/// nestri-launcher — Nestri Windows GUI launcher
///
/// On first run: shows a setup dialog (room name input)
/// After setup: shows system tray icon + stream URL popup
/// Starts nestri-server.exe + nestri-relay.exe in background
/// Stream URL: https://xtreme-gaming.pages.dev/play?room=<room>

#![windows_subsystem = "windows"]

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const WEB_CLIENT: &str = "https://xtreme-gaming.pages.dev";
const DEFAULT_RELAY: &str = "/dnsaddr/relay.dathorse.com/p2p/12D3KooWPK4v5wKYNYx9oXWjqLM8Xix6nm13o91j1Feqq98fLBsw";

// Windows message IDs
const WM_TRAYICON: u32 = 0x0400 + 1; // WM_USER + 1
const IDM_OPEN_BROWSER: u32 = 100;
const IDM_COPY_URL: u32    = 101;
const IDM_STOP: u32        = 102;
const IDM_START: u32       = 103;
const IDM_EXIT: u32        = 104;

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Shell::*,
    Win32::System::DataExchange::*,
};

struct AppState {
    install_dir: PathBuf,
    room:        String,
    relay_url:   String,
    stream_url:  String,
    server:      Option<Child>,
    relay:       Option<Child>,
}

impl AppState {
    fn new(install_dir: PathBuf) -> Self {
        let env = load_env(&install_dir);
        let room = env.get("NESTRI_ROOM")
            .cloned()
            .unwrap_or_else(|| "nestri-windows".to_string());
        let relay_url = env.get("NESTRI_RELAY_URL")
            .cloned()
            .unwrap_or_else(|| DEFAULT_RELAY.to_string());
        let stream_url = format!("{}/play?room={}", WEB_CLIENT, room);

        Self { install_dir, room, relay_url, stream_url, server: None, relay: None }
    }

    fn start(&mut self) {
        let env = load_env(&self.install_dir);

        // Start relay first
        let relay_bin = self.install_dir.join("nestri-relay.exe");
        if relay_bin.exists() {
            if let Ok(child) = Command::new(&relay_bin)
                .envs(&env).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
            {
                self.relay = Some(child);
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
        }

        // Start server
        let server_bin = self.install_dir.join("nestri-server.exe");
        if server_bin.exists() {
            if let Ok(child) = Command::new(&server_bin)
                .envs(&env).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
            {
                self.server = Some(child);
            }
        }
    }

    fn stop(&mut self) {
        if let Some(mut p) = self.server.take() { let _ = p.kill(); }
        if let Some(mut p) = self.relay.take()  { let _ = p.kill(); }
    }

    fn is_running(&mut self) -> bool {
        self.server.as_mut().map_or(false, |p| matches!(p.try_wait(), Ok(None)))
    }
}

fn load_env(dir: &PathBuf) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let path = dir.join("nestri.env");
    if let Ok(f) = fs::File::open(&path) {
        for line in BufReader::new(f).lines().flatten() {
            let line = line.trim().to_string();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

fn save_env(dir: &PathBuf, room: &str, relay: &str) {
    let path = dir.join("nestri.env");
    let content = format!(
        "NESTRI_ROOM={room}\n\
         NESTRI_RELAY_URL={relay}\n\
         NESTRI_GPU_VENDOR=nvidia\n\
         NESTRI_VIDEO_CODEC=h264\n\
         NESTRI_ENCODER_TYPE=hardware\n\
         NESTRI_LATENCY=lowest-latency\n\
         NESTRI_RATE_CONTROL=cbr:8000\n\
         NESTRI_FRAMERATE=60\n\
         NESTRI_MONITOR=0\n\
         NESTRI_AUDIO_BITRATE=128\n"
    );
    if let Ok(mut f) = fs::File::create(&path) {
        let _ = f.write_all(content.as_bytes());
    }
}

fn is_first_run(dir: &PathBuf) -> bool {
    !dir.join("nestri.env").exists()
}

#[cfg(target_os = "windows")]
fn show_first_run_dialog(install_dir: &PathBuf) -> Option<(String, String)> {
    // Simple input-box style dialog asking for room name
    // Returns (room_name, relay_url)
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        // Use a simple dialog via TaskDialog for room name input
        // For simplicity, we'll use InputBox equivalent via a custom dialog
        // Quick approach: ask via a message box sequence then use a default
        let msg = format!(
            "Welcome to Nestri!\n\n\
             Your stream URL will be:\n\
             {WEB_CLIENT}/play?room=YOUR-ROOM-NAME\n\n\
             Enter a room name (letters and hyphens only).\n\
             This identifies YOUR PC on the network.\n\n\
             Leave blank to use: nestri-windows"
        );

        // We use a simple named pipe trick — show a dialog and capture input
        // Fallback: use a WS_EX_TOOLWINDOW popup with an Edit control
        let room = show_input_dialog(hinstance, "Nestri Setup", &msg, "nestri-windows")?;
        let relay = DEFAULT_RELAY.to_string();
        Some((room, relay))
    }
}

#[cfg(target_os = "windows")]
unsafe fn show_input_dialog(
    hinstance: windows::Win32::Foundation::HMODULE,
    title:     &str,
    message:   &str,
    default:   &str,
) -> Option<String> {
    // Create a simple modal dialog with an Edit control
    // We define it as a popup window with manual controls

    let class_name: Vec<u16> = "NestriInputDlg\0".encode_utf16().collect();
    let wc = WNDCLASSW {
        lpfnWndProc:   Some(input_dialog_proc),
        hInstance:     hinstance.into(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        hCursor:       LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        ..Default::default()
    };
    RegisterClassW(&wc);

    let title_w:   Vec<u16> = format!("{title}\0").encode_utf16().collect();

    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
        PCWSTR(class_name.as_ptr()),
        PCWSTR(title_w.as_ptr()),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        CW_USEDEFAULT, CW_USEDEFAULT, 520, 280,
        None, None, hinstance, None,
    ).ok()?;

    // Store context in window
    let ctx = Box::new(DialogContext {
        message: message.to_string(),
        default: default.to_string(),
        result:  None,
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize);

    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        if !IsDialogMessageW(hwnd, &msg).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if !IsWindow(hwnd).as_bool() {
            break;
        }
    }

    // Retrieve result from context (if window was destroyed with a result)
    // We use a global static for simplicity
    Some(DIALOG_RESULT.lock().unwrap().clone().unwrap_or_else(|| default.to_string()))
}

static DIALOG_RESULT: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

struct DialogContext {
    message: String,
    default: String,
    result:  Option<String>,
}

const ID_EDIT: u32   = 200;
const ID_OK: u32     = 201;
const ID_CANCEL: u32 = 202;

#[cfg(target_os = "windows")]
unsafe extern "system" fn input_dialog_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            let (message, default) = if ctx_ptr != 0 {
                let ctx = &*(ctx_ptr as *const DialogContext);
                (ctx.message.clone(), ctx.default.clone())
            } else {
                ("Enter room name:".to_string(), "nestri-windows".to_string())
            };

            let hinstance = GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) as isize;

            // Static text label
            let msg_w: Vec<u16> = format!("{message}\0").encode_utf16().collect();
            CreateWindowExW(WS_EX_TRANSPARENT, PCWSTR("STATIC\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                PCWSTR(msg_w.as_ptr()), WS_CHILD | WS_VISIBLE,
                20, 10, 480, 150, hwnd, None, HMODULE(hinstance), None);

            // Edit control
            let def_w: Vec<u16> = format!("{default}\0").encode_utf16().collect();
            CreateWindowExW(WS_EX_CLIENTEDGE, PCWSTR("EDIT\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                PCWSTR(def_w.as_ptr()), WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(0x0080), // ES_AUTOHSCROLL
                20, 165, 470, 28, hwnd, HMENU(ID_EDIT as isize), HMODULE(hinstance), None);

            // OK button
            CreateWindowExW(Default::default(), PCWSTR("BUTTON\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                PCWSTR("Start Streaming\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x1), // BS_DEFPUSHBUTTON
                20, 210, 160, 32, hwnd, HMENU(ID_OK as isize), HMODULE(hinstance), None);

            // Cancel button
            CreateWindowExW(Default::default(), PCWSTR("BUTTON\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                PCWSTR("Cancel\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
                WS_CHILD | WS_VISIBLE,
                200, 210, 100, 32, hwnd, HMENU(ID_CANCEL as isize), HMODULE(hinstance), None);

            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as u32;
            if cmd == ID_OK {
                // Get text from edit control
                let hedit = GetDlgItem(hwnd, ID_EDIT as i32).unwrap_or_default();
                let mut buf = vec![0u16; 256];
                let len = GetWindowTextW(hedit, &mut buf);
                let text = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
                let result = if text.is_empty() { "nestri-windows".to_string() } else { text };
                *DIALOG_RESULT.lock().unwrap() = Some(result);
                DestroyWindow(hwnd);
            } else if cmd == ID_CANCEL {
                *DIALOG_RESULT.lock().unwrap() = None;
                DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(target_os = "windows")]
fn show_balloon(hwnd: HWND, title: &str, msg: &str) {
    unsafe {
        let tip: Vec<u16>   = "Nestri\0".encode_utf16().collect();
        let title_w: Vec<u16> = format!("{title}\0").encode_utf16().collect();
        let msg_w: Vec<u16>   = format!("{msg}\0").encode_utf16().collect();

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd:   hwnd,
            uID:    1,
            uFlags: NIF_INFO,
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };
        let tl = title_w.len().min(nid.szInfoTitle.len());
        nid.szInfoTitle[..tl].copy_from_slice(&title_w[..tl]);
        let ml = msg_w.len().min(nid.szInfo.len());
        nid.szInfo[..ml].copy_from_slice(&msg_w[..ml]);

        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

#[cfg(target_os = "windows")]
fn open_browser(url: &str) {
    unsafe {
        let url_w: Vec<u16> = format!("{url}\0").encode_utf16().collect();
        ShellExecuteW(
            None,
            PCWSTR("open\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
            PCWSTR(url_w.as_ptr()),
            None, None,
            SW_SHOW,
        );
    }
}

#[cfg(target_os = "windows")]
fn copy_to_clipboard(text: &str) {
    unsafe {
        if OpenClipboard(None).is_ok() {
            EmptyClipboard();
            let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = text_w.len() * 2;
            let hmem = windows::Win32::System::Memory::GlobalAlloc(
                windows::Win32::System::Memory::GMEM_MOVEABLE,
                bytes,
            ).unwrap();
            let ptr = windows::Win32::System::Memory::GlobalLock(hmem);
            std::ptr::copy_nonoverlapping(text_w.as_ptr(), ptr as *mut u16, text_w.len());
            windows::Win32::System::Memory::GlobalUnlock(hmem);
            SetClipboardData(13, HANDLE(hmem.0 as isize)); // CF_UNICODETEXT = 13
            CloseClipboard();
        }
    }
}

#[cfg(target_os = "windows")]
fn main() {
    let install_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // ── First-run setup ───────────────────────────────────────────────────────
    if is_first_run(&install_dir) {
        if let Some((room, relay)) = show_first_run_dialog(&install_dir) {
            save_env(&install_dir, &room, &relay);
        } else {
            // User cancelled — use defaults
            save_env(&install_dir, "nestri-windows", DEFAULT_RELAY);
        }
    }

    let state = Arc::new(Mutex::new(AppState::new(install_dir.clone())));
    let stream_url = state.lock().unwrap().stream_url.clone();
    let room       = state.lock().unwrap().room.clone();

    // Auto-start streaming
    state.lock().unwrap().start();

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        // Register window class
        let class_w: Vec<u16> = "NestriLauncher\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            lpfnWndProc:   Some(tray_wnd_proc),
            hInstance:     hinstance.into(),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_w.as_ptr()),
            PCWSTR("Nestri\0".encode_utf16().collect::<Vec<_>>().as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
            HWND_MESSAGE, None, hinstance, None,
        ).unwrap();

        // Store state pointer
        let state_raw = Arc::into_raw(state.clone()) as isize;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_raw);

        // Create tray icon
        let tip: Vec<u16> = format!("Nestri — {room}\0").encode_utf16().collect();
        let mut nid = NOTIFYICONDATAW {
            cbSize:           std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd:             hwnd,
            uID:              1,
            uFlags:           NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon:            LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        let tl = tip.len().min(nid.szTip.len());
        nid.szTip[..tl].copy_from_slice(&tip[..tl]);
        Shell_NotifyIconW(NIM_ADD, &nid);

        // Show balloon with stream URL immediately on startup
        std::thread::sleep(std::time::Duration::from_secs(2));
        show_balloon(hwnd, "🎮 Nestri is Streaming!",
            &format!("Your stream URL:\n{stream_url}\n\nRight-click tray icon for options."));

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        Shell_NotifyIconW(NIM_DELETE, &nid);
        let state = Arc::from_raw(state_raw as *const Mutex<AppState>);
        state.lock().unwrap().stop();
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn tray_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        x if x == WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP as u32 || event == WM_LBUTTONUP as u32 {
                let state_raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                let (running, stream_url) = if state_raw != 0 {
                    let state = &*(state_raw as *const Mutex<AppState>);
                    let mut s = state.lock().unwrap();
                    (s.is_running(), s.stream_url.clone())
                } else { (false, String::new()) };

                let hmenu = CreatePopupMenu().unwrap();

                // Header (greyed, non-clickable)
                let hdr: Vec<u16> = "🎮 Nestri Gaming\0".encode_utf16().collect();
                AppendMenuW(hmenu, MF_STRING | MF_GRAYED, 0, PCWSTR(hdr.as_ptr()));
                AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

                // Stream URL item (greyed display)
                let url_item: Vec<u16> = format!("📺  {stream_url}\0").encode_utf16().collect();
                AppendMenuW(hmenu, MF_STRING | MF_GRAYED, 0, PCWSTR(url_item.as_ptr()));
                AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

                // Actions
                let open_w: Vec<u16> = "🌐  Open in Browser\0".encode_utf16().collect();
                let copy_w: Vec<u16> = "📋  Copy Stream URL\0".encode_utf16().collect();
                AppendMenuW(hmenu, MF_STRING, IDM_OPEN_BROWSER as usize, PCWSTR(open_w.as_ptr()));
                AppendMenuW(hmenu, MF_STRING, IDM_COPY_URL    as usize, PCWSTR(copy_w.as_ptr()));
                AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

                if running {
                    let stop_w: Vec<u16> = "■  Stop Streaming\0".encode_utf16().collect();
                    AppendMenuW(hmenu, MF_STRING, IDM_STOP as usize, PCWSTR(stop_w.as_ptr()));
                } else {
                    let start_w: Vec<u16> = "▶  Start Streaming\0".encode_utf16().collect();
                    AppendMenuW(hmenu, MF_STRING, IDM_START as usize, PCWSTR(start_w.as_ptr()));
                }

                let exit_w: Vec<u16> = "✕  Exit Nestri\0".encode_utf16().collect();
                AppendMenuW(hmenu, MF_STRING, IDM_EXIT as usize, PCWSTR(exit_w.as_ptr()));

                SetForegroundWindow(hwnd);
                let mut pt = windows::Win32::Foundation::POINT::default();
                windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
                TrackPopupMenu(hmenu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, None);
                DestroyMenu(hmenu);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as u32;
            let state_raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if state_raw != 0 {
                let state = &*(state_raw as *const Mutex<AppState>);
                match cmd {
                    x if x == IDM_OPEN_BROWSER => {
                        let url = state.lock().unwrap().stream_url.clone();
                        open_browser(&url);
                    }
                    x if x == IDM_COPY_URL => {
                        let url = state.lock().unwrap().stream_url.clone();
                        copy_to_clipboard(&url);
                        show_balloon(hwnd, "Copied!", "Stream URL copied to clipboard.");
                    }
                    x if x == IDM_STOP => {
                        state.lock().unwrap().stop();
                    }
                    x if x == IDM_START => {
                        state.lock().unwrap().start();
                    }
                    x if x == IDM_EXIT => {
                        state.lock().unwrap().stop();
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("nestri-launcher is Windows-only.");
}
