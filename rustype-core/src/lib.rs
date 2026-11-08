//! Rustype core: Vietnamese Telex composition engine, pure and platform-independent.
//!
//! This crate has no I/O, no platform dependencies, and no external crates.
//! It can be reused by any input method framework: IBus, Windows IME, macOS
//! InputMethodKit, and others.

pub mod ffi;
pub mod tables;
pub mod telex;
