//! Windows System Tray (Shell_NotifyIcon) and context menu implementation.

#[cfg(windows)]
pub mod win {
    use crate::config::WinConfig;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_TRAYICON: u32 = WM_USER + 100;
    const ID_TRAY_TOGGLE: usize = 2001;
    const ID_TRAY_TELEX: usize = 2002;
    const ID_TRAY_VNI: usize = 2003;
    const ID_TRAY_AUTO_RESTORE: usize = 2004;
    const ID_TRAY_MODERN: usize = 2005;
    const ID_TRAY_EXIT: usize = 2006;

    pub struct TrayManager {
        pub hwnd: HWND,
        pub config: WinConfig,
    }
    unsafe impl Send for TrayManager {}

    static TRAY_STATE: Mutex<Option<TrayManager>> = Mutex::new(None);

    /// Initializes the system tray icon.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle.
    pub unsafe fn init_tray(hwnd: HWND, config: WinConfig) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);

        let tip = if config.enabled {
            "Rustype - Tiếng Việt (VN)\0"
        } else {
            "Rustype - English (EN)\0"
        };
        for (i, c) in tip.encode_utf16().enumerate() {
            if i < nid.szTip.len() {
                nid.szTip[i] = c;
            }
        }

        Shell_NotifyIconW(NIM_ADD, &nid);

        let mut lock = TRAY_STATE.lock().unwrap();
        *lock = Some(TrayManager { hwnd, config });
    }

    /// Updates the tooltip text of the system tray icon.
    ///
    /// # Safety
    /// Must be called from the UI thread that owns the tray icon.
    pub unsafe fn update_tray_tip(enabled: bool) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.uID = 1;
        nid.uFlags = NIF_TIP;

        let lock = TRAY_STATE.lock().unwrap();
        if let Some(mgr) = lock.as_ref() {
            nid.hWnd = mgr.hwnd;
            let tip = if enabled {
                "Rustype - Tiếng Việt (VN)\0"
            } else {
                "Rustype - English (EN)\0"
            };
            for (i, c) in tip.encode_utf16().enumerate() {
                if i < nid.szTip.len() {
                    nid.szTip[i] = c;
                }
            }
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Removes the tray icon from the taskbar notification area.
    ///
    /// # Safety
    /// Must be called before window destruction.
    pub unsafe fn remove_tray() {
        let mut lock = TRAY_STATE.lock().unwrap();
        if let Some(mgr) = lock.take() {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = mgr.hwnd;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }

    /// Displays the context menu for the system tray icon.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle.
    pub unsafe fn show_context_menu(hwnd: HWND, cfg: &WinConfig) {
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);

        let menu = CreatePopupMenu();

        let add_item = |id: usize, text: &str, checked: bool| {
            let mut flags = MF_STRING;
            if checked {
                flags |= MF_CHECKED;
            }
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            AppendMenuW(menu, flags, id, wide.as_ptr());
        };

        add_item(ID_TRAY_TOGGLE, "Bật tiếng Việt (Ctrl+Shift)", cfg.enabled);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

        let is_telex = cfg.input_method == rustype_core::telex::InputMethod::Telex;
        add_item(ID_TRAY_TELEX, "Kiểu gõ Telex", is_telex);
        add_item(ID_TRAY_VNI, "Kiểu gõ VNI", !is_telex);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

        add_item(
            ID_TRAY_AUTO_RESTORE,
            "Tự động khôi phục từ tiếng Anh",
            cfg.auto_restore_english,
        );
        add_item(
            ID_TRAY_MODERN,
            "Đặt dấu kiểu mới (hoà, khoẻ)",
            cfg.modern_orthography,
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

        add_item(ID_TRAY_EXIT, "Thoát Rustype", false);

        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
    }
}
