//! Rustype for Windows: Lightweight, high-performance Vietnamese input method.
//!
//! Provides both low-level keyboard hook (Unikey/EVKey style) for universal app/game compatibility
//! and an extensible foundation for Windows Text Services Framework (TSF).

pub mod config;
pub mod hook;
pub mod tray;

pub use config::WinConfig;
