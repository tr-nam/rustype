//! Windows System Tray (Shell_NotifyIcon), context menu and tray window procedure.

#[cfg(windows)]
pub mod win {
    use crate::config::WinConfig;
    use crate::hook::win::{modify_config, snapshot_config, WM_CONFIG_CHANGED};
    use rustype_core::telex::InputMethod;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
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
    }
    unsafe impl Send for TrayManager {}

    static TRAY_STATE: Mutex<Option<TrayManager>> = Mutex::new(None);

    /// Writes the tooltip text for the given enabled state into the notification data.
    fn set_tip(nid: &mut NOTIFYICONDATAW, enabled: bool) {
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
    }

    /// Initializes the system tray icon.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle.
    pub unsafe fn init_tray(hwnd: HWND, enabled: bool) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        set_tip(&mut nid, enabled);

        Shell_NotifyIconW(NIM_ADD, &nid);

        let mut lock = TRAY_STATE.lock().unwrap();
        *lock = Some(TrayManager { hwnd });
    }

    /// Updates the tooltip text of the system tray icon.
    ///
    /// # Safety
    /// Must be called from the UI thread that owns the tray icon.
    pub unsafe fn update_tray_tip(enabled: bool) {
        let Some(hwnd) = TRAY_STATE
            .lock()
            .ok()
            .and_then(|lock| lock.as_ref().map(|mgr| mgr.hwnd))
        else {
            return;
        };

        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.uID = 1;
        nid.uFlags = NIF_TIP;
        nid.hWnd = hwnd;
        set_tip(&mut nid, enabled);
        Shell_NotifyIconW(NIM_MODIFY, &nid);
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

        let is_telex = cfg.input_method == InputMethod::Telex;
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

        // Required so the menu dismisses when the user clicks elsewhere.
        SetForegroundWindow(hwnd);
        // Selected commands are delivered to `hwnd` as WM_COMMAND.
        TrackPopupMenu(
            menu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }

    /// Re-reads the engine configuration, updates the tray tip and saves it to disk.
    ///
    /// # Safety
    /// Must be called from the UI thread that owns the tray icon.
    pub unsafe fn apply_config_change() {
        let Some(cfg) = snapshot_config() else {
            return;
        };
        update_tray_tip(cfg.enabled);
        if let Err(err) = cfg.save() {
            eprintln!("Failed to save Rustype config: {err}");
        }
    }

    /// Applies a configuration change, then refreshes the tray and saves.
    unsafe fn change_config(change: impl FnOnce(&mut WinConfig)) {
        if modify_config(change).is_some() {
            apply_config_change();
        }
    }

    /// Runs the action for a menu command ID.
    unsafe fn handle_command(id: usize) {
        match id {
            ID_TRAY_TOGGLE => change_config(|cfg| cfg.enabled = !cfg.enabled),
            ID_TRAY_TELEX => change_config(|cfg| cfg.input_method = InputMethod::Telex),
            ID_TRAY_VNI => change_config(|cfg| cfg.input_method = InputMethod::Vni),
            ID_TRAY_AUTO_RESTORE => {
                change_config(|cfg| cfg.auto_restore_english = !cfg.auto_restore_english)
            }
            ID_TRAY_MODERN => change_config(|cfg| cfg.modern_orthography = !cfg.modern_orthography),
            ID_TRAY_EXIT => {
                remove_tray();
                PostQuitMessage(0);
            }
            _ => {}
        }
    }

    /// Window procedure for the hidden message window.
    ///
    /// Handles tray icon clicks, context menu commands and configuration change notifications
    /// posted by the keyboard hook.
    ///
    /// # Safety
    /// Called by the system for windows of the Rustype window class.
    pub unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAYICON => {
                // With the default notify-icon version, lParam carries the mouse message.
                match (lparam as u32) & 0xFFFF {
                    WM_LBUTTONUP => change_config(|cfg| cfg.enabled = !cfg.enabled),
                    WM_RBUTTONUP => {
                        if let Some(cfg) = snapshot_config() {
                            show_context_menu(hwnd, &cfg);
                        }
                    }
                    _ => {}
                }
                0
            }
            WM_COMMAND => {
                // LOWORD of wParam is the menu item ID.
                handle_command(wparam & 0xFFFF);
                0
            }
            WM_CONFIG_CHANGED => {
                apply_config_change();
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
