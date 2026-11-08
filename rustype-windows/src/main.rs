//! Rustype Windows Input Method Engine Daemon
//!
//! Entry point for Windows execution.

#[cfg(windows)]
fn main() {
    use rustype_windows::config::WinConfig;
    use rustype_windows::hook::win::{install_hook, set_notify_window, uninstall_hook};
    use rustype_windows::tray::win::{init_tray, remove_tray, tray_wnd_proc};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let config = WinConfig::load();
    let enabled = config.enabled;
    if !install_hook(config) {
        eprintln!("Failed to install low-level keyboard hook.");
        return;
    }

    unsafe {
        // Create hidden message-only window for system tray events
        let class_name = "RustypeMessageWindowClass\0"
            .encode_utf16()
            .collect::<Vec<u16>>();
        let h_instance =
            windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());

        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(tray_wnd_proc);
        wc.hInstance = h_instance;
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            h_instance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            eprintln!("Failed to create the tray message window.");
            uninstall_hook();
            return;
        }

        set_notify_window(hwnd);
        init_tray(hwnd, enabled);

        // Standard Win32 Message Loop; exits when the tray menu posts WM_QUIT.
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        remove_tray();
    }

    uninstall_hook();
}

#[cfg(not(windows))]
fn main() {
    println!("Rustype Windows daemon (rustype-windows) is built for Microsoft Windows.");
    println!("To compile for Windows on Linux, use:");
    println!("  cargo build --target x86_64-pc-windows-gnu -p rustype-windows");
}
