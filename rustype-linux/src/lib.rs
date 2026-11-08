//! rustype-linux: an IBus-protocol bus for libibus clients (GNOME Shell, GTK and Qt modules)
//! that runs the Rustype engine in-process. One process replaces ibus-daemon and the engine.

pub mod config;
pub mod ime;

mod addrfile;
mod bus;
mod cli;
mod dbus;
mod doctor;
mod props;
mod sasl;
mod server;
mod service;
mod sys;
mod wire;

/// Runs the `rustype-linux` command line: the daemon, or one of its tools.
pub fn run_cli() {
    cli::main();
}
