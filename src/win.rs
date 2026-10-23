//! Windows-only helpers: the foreground app (automatic profiles), Raw Input key timing
//! (polling-rate test), the tray icon, and hiding/showing the main window.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, RAWINPUTDEVICE,
    RAWINPUTHEADER, RAWKEYBOARD, RIDEV_INPUTSINK, RIDI_DEVICENAME, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetForegroundWindow,
    GetMessageW, GetWindowTextW, GetWindowThreadProcessId, PostThreadMessageW, RegisterClassW,
    GetSystemMetrics, LoadImageW, RegisterWindowMessageW, SetForegroundWindow, ShowWindow, TrackPopupMenu, HICON,
    IMAGE_ICON, MF_STRING, MSG, SM_CXSMICON, SM_CYSMICON, SW_HIDE,
    SW_SHOW, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_INPUT, WM_LBUTTONUP, WM_QUIT,
    WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---- foreground app -------------------------------------------------------------------

/// The foreground window's exe file name (e.g. `VALORANT-Win64-Shipping.exe`) and title.
pub fn foreground_app() -> Option<(String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let exe = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        let mut title = [0u16; 256];
        let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        Some((exe, String::from_utf16_lossy(&title[..n.max(0) as usize])))
    }
}

/// Reports the foreground app whenever its exe changes, polling a few times a second.
pub struct ForegroundWatcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ForegroundWatcher {
    pub fn start(sender: async_channel::Sender<(String, String)>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let own = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        let thread = std::thread::spawn(move || {
            let mut last = String::new();
            while !s.load(Ordering::Relaxed) {
                if let Some((exe, title)) = foreground_app() {
                    // This app coming to the front shouldn't switch profiles.
                    let ours = own.as_deref() == Some(exe.to_lowercase().as_str());
                    if exe != last && !ours {
                        last = exe.clone();
                        if sender.try_send((exe, title)).is_err() {
                            break;
                        }
                    } else if ours {
                        last.clear();
                    }
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for ForegroundWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ---- hidden message windows ---------------------------------------------------------------

/// A never-shown window on the current thread, for Raw Input and tray messages.
unsafe fn hidden_window(class: &str, proc_: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) -> HWND {
    let name = wide(class);
    let instance = GetModuleHandleW(std::ptr::null());
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc_),
        hInstance: instance,
        lpszClassName: name.as_ptr(),
        ..std::mem::zeroed()
    };
    // Fails harmlessly when the class is already registered by an earlier run.
    RegisterClassW(&wc);
    CreateWindowExW(
        0,
        name.as_ptr(),
        name.as_ptr(),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    )
}

fn pump_messages() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    }
}

/// A thread running a message loop; dropping it ends the loop.
struct MessageThread {
    id: u32,
    thread: Option<JoinHandle<()>>,
}

impl MessageThread {
    fn spawn(run: impl FnOnce() + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let _ = tx.send(unsafe { GetCurrentThreadId() });
            run();
        });
        Self {
            id: rx.recv().unwrap_or(0),
            thread: Some(thread),
        }
    }
}

impl Drop for MessageThread {
    fn drop(&mut self) {
        // Retry: the thread may not have created its message queue yet.
        for _ in 0..50 {
            if unsafe { PostThreadMessageW(self.id, WM_QUIT, 0, 0) } != 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ---- Raw Input timing -------------------------------------------------------------------------

pub enum RawEvent {
    Ready,
    Error(String),
    /// Microsecond timestamp of a key change.
    Key(f64),
}

struct RawSink {
    sender: async_channel::Sender<RawEvent>,
    filter: Option<String>,
    us_per_tick: f64,
    names: Vec<(isize, bool)>,
}

thread_local! {
    static RAW: RefCell<Option<RawSink>> = const { RefCell::new(None) };
}

unsafe fn device_matches(device: windows_sys::Win32::Foundation::HANDLE, filter: &str) -> bool {
    let mut size = 512u32;
    let mut buf = [0u16; 512];
    GetRawInputDeviceInfoW(device, RIDI_DEVICENAME, buf.as_mut_ptr().cast(), &mut size);
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).to_uppercase().contains(filter)
}

unsafe extern "system" fn raw_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        let mut ticks = 0i64;
        QueryPerformanceCounter(&mut ticks);
        let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
        let mut buf = [0u8; 128];
        let mut size = buf.len() as u32;
        let got = GetRawInputData(lparam as _, RID_INPUT, buf.as_mut_ptr().cast(), &mut size, header_size);
        if got != u32::MAX && got as usize >= header_size as usize + std::mem::size_of::<RAWKEYBOARD>() {
            let header: RAWINPUTHEADER = std::ptr::read_unaligned(buf.as_ptr().cast());
            if header.dwType == RIM_TYPEKEYBOARD {
                RAW.with(|r| {
                    if let Some(sink) = r.borrow_mut().as_mut() {
                        let key = header.hDevice as isize;
                        let ok = match (&sink.filter, sink.names.iter().find(|(d, _)| *d == key)) {
                            (None, _) => true,
                            (Some(_), Some(&(_, ok))) => ok,
                            (Some(f), None) => {
                                let ok = device_matches(header.hDevice, f);
                                sink.names.push((key, ok));
                                ok
                            }
                        };
                        if ok {
                            let _ = sink.sender.try_send(RawEvent::Key(ticks as f64 * sink.us_per_tick));
                        }
                    }
                });
            }
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Key-change timestamps from Raw Input, for the keyboards whose device path contains
/// `filter` (all keyboards when None). Runs on its own thread until dropped.
pub struct RawCapture {
    _thread: MessageThread,
}

impl RawCapture {
    pub fn start(filter: Option<&str>, sender: async_channel::Sender<RawEvent>) -> Self {
        let filter = filter.map(|f| f.to_uppercase());
        let thread = MessageThread::spawn(move || unsafe {
            let mut freq = 0i64;
            QueryPerformanceFrequency(&mut freq);
            let hwnd = hidden_window("MonsgeekAppRawInput", raw_proc);
            let device = RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            };
            if hwnd.is_null()
                || RegisterRawInputDevices(&device, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0
            {
                let _ = sender.try_send(RawEvent::Error("Raw Input registration failed".into()));
                return;
            }
            let _ = sender.try_send(RawEvent::Ready);
            RAW.with(|r| {
                *r.borrow_mut() = Some(RawSink {
                    sender,
                    filter,
                    us_per_tick: 1e6 / freq.max(1) as f64,
                    names: Vec::new(),
                })
            });
            pump_messages();
            RAW.with(|r| *r.borrow_mut() = None);
            DestroyWindow(hwnd);
        });
        Self { _thread: thread }
    }
}

// ---- tray icon ---------------------------------------------------------------------------------

pub enum TrayEvent {
    Open,
    Quit,
}

const TRAY_MESSAGE: u32 = WM_APP + 1;
const MENU_OPEN: usize = 1;
const MENU_QUIT: usize = 2;

thread_local! {
    static TRAY: RefCell<Option<async_channel::Sender<TrayEvent>>> = const { RefCell::new(None) };
    /// The icon as added, to add it again when Explorer restarts ("TaskbarCreated").
    static TRAY_ICON: RefCell<Option<(u32, NOTIFYICONDATAW)>> = const { RefCell::new(None) };
}

unsafe extern "system" fn tray_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let readd = TRAY_ICON.with(|t| match t.borrow().as_ref() {
        Some((created, data)) if *created != 0 && msg == *created => Some(*data),
        _ => None,
    });
    if let Some(data) = readd {
        Shell_NotifyIconW(NIM_ADD, &data);
        return 0;
    }
    if msg == TRAY_MESSAGE {
        let event = match (lparam & 0xffff) as u32 {
            WM_LBUTTONUP => Some(TrayEvent::Open),
            WM_RBUTTONUP => {
                let menu = CreatePopupMenu();
                AppendMenuW(menu, MF_STRING, MENU_OPEN, wide("Open Monsgeek App").as_ptr());
                AppendMenuW(menu, MF_STRING, MENU_QUIT, wide("Quit").as_ptr());
                let mut p = POINT { x: 0, y: 0 };
                GetCursorPos(&mut p);
                // Without this the menu doesn't close when clicking elsewhere.
                SetForegroundWindow(hwnd);
                let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, std::ptr::null());
                DestroyMenu(menu);
                match cmd as usize {
                    MENU_OPEN => Some(TrayEvent::Open),
                    MENU_QUIT => Some(TrayEvent::Quit),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(e) = event {
            TRAY.with(|t| {
                if let Some(s) = t.borrow().as_ref() {
                    let _ = s.try_send(e);
                }
            });
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// The exe's embedded icon (resource 1, see build.rs) at the tray's size.
unsafe fn app_icon() -> HICON {
    LoadImageW(
        GetModuleHandleW(std::ptr::null()),
        1 as _,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        0,
    ) as HICON
}

/// A notification-area icon: left click opens the window, the menu offers Open and Quit.
pub struct Tray {
    _thread: MessageThread,
}

impl Tray {
    pub fn start(tooltip: &str, sender: async_channel::Sender<TrayEvent>) -> Self {
        let tip = tooltip.to_string();
        let thread = MessageThread::spawn(move || unsafe {
            let hwnd = hidden_window("MonsgeekAppTray", tray_proc);
            if hwnd.is_null() {
                return;
            }
            TRAY.with(|t| *t.borrow_mut() = Some(sender));
            let icon = app_icon();
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            data.uCallbackMessage = TRAY_MESSAGE;
            data.hIcon = icon;
            for (dst, src) in data.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
                *dst = src;
            }
            Shell_NotifyIconW(NIM_ADD, &data);
            // Explorer broadcasts this to top-level windows after it restarts, with an empty tray.
            let created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            TRAY_ICON.with(|t| *t.borrow_mut() = Some((created, data)));
            pump_messages();
            TRAY_ICON.with(|t| *t.borrow_mut() = None);
            Shell_NotifyIconW(NIM_DELETE, &data);
            // LoadImageW without LR_SHARED returns our own copy, so it must be destroyed.
            if !icon.is_null() {
                DestroyIcon(icon);
            }
            DestroyWindow(hwnd);
            TRAY.with(|t| *t.borrow_mut() = None);
        });
        Self { _thread: thread }
    }
}

// ---- main window --------------------------------------------------------------------------------

/// Hide the window to the tray, or bring it back to the front.
pub fn show_window(hwnd: isize, show: bool) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = hwnd as HWND;
        ShowWindow(h, if show { SW_SHOW } else { SW_HIDE });
        if show {
            SetForegroundWindow(h);
        }
    }
}

/// The Win32 handle behind a GPUI window.
pub fn hwnd_of(window: &impl raw_window_handle::HasWindowHandle) -> isize {
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(raw_window_handle::RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_capture_starts_and_stops() {
        let (tx, rx) = async_channel::unbounded();
        let capture = RawCapture::start(Some("VID_3151"), tx);
        let first = rx.recv_blocking().unwrap();
        assert!(matches!(first, RawEvent::Ready));
        drop(capture);
        assert!(foreground_app().is_none_or(|(exe, _)| exe.ends_with(".exe") || !exe.is_empty()));
    }
}
