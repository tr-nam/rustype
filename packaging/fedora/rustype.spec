Name:           rustype
Version:        0.2.1
Release:        1%{?dist}
Summary:        Lightweight Vietnamese input method for GNOME (IBus)

License:        MIT
URL:            https://namtr.dev
Source0:        https://github.com/tr-nam/rustype/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.70.0

Requires:       ibus
Requires:       gnome-shell
Requires:       python3
Requires:       python3-gobject
Requires:       libadwaita
Requires:       gtk4

%description
Rustype is an ultra-lightweight, high-performance, and privacy-focused
Vietnamese input method for GNOME. rustype-linux speaks the IBus protocol
itself and replaces ibus-daemon for the GNOME session. Written in pure Rust
with zero external GUI dependencies in core, maintaining a minimal ~2.5 MB
RAM footprint. Supports Telex and VNI with modern GTK4 Libadwaita settings.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked -p rustype-linux

%install
# Binaries
install -Dm755 target/release/rustype-linux %{buildroot}%{_libexecdir}/rustype-linux
mkdir -p %{buildroot}%{_bindir}
ln -sf ../libexec/rustype-linux %{buildroot}%{_bindir}/rustype-linux
install -Dm755 tools/rustype-settings %{buildroot}%{_bindir}/rustype-settings

# Vendor drop-in: the GNOME IBus unit runs rustype-linux instead of ibus-daemon
install -Dm644 packaging/systemd/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf %{buildroot}%{_prefix}/lib/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf

# Desktop entry
install -Dm644 tools/dev.namtr.RustypeSettings.desktop %{buildroot}%{_datadir}/applications/dev.namtr.RustypeSettings.desktop

# Icons
for s in 16 32 48 64 128 256 512; do
    install -Dm644 assets/icons/${s}x${s}/rustype.png %{buildroot}%{_datadir}/icons/hicolor/${s}x${s}/apps/rustype.png
    ln -sf rustype.png %{buildroot}%{_datadir}/icons/hicolor/${s}x${s}/apps/dev.namtr.RustypeSettings.png
done
install -Dm644 assets/icons/scalable/rustype.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/rustype.svg
ln -sf rustype.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/dev.namtr.RustypeSettings.svg

# GNOME Shell extension
mkdir -p %{buildroot}%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev
cp -a gnome-shell-extension/rustype-focus@namtr.dev/* %{buildroot}%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev/

# Man pages
install -Dm644 docs/man/rustype-linux.1 %{buildroot}%{_mandir}/man1/rustype-linux.1
install -Dm644 docs/man/rustype-settings.1 %{buildroot}%{_mandir}/man1/rustype-settings.1

%files
%license LICENSE
%doc README.md CHANGELOG.md docs/PRIVACY.md
%{_libexecdir}/rustype-linux
%{_bindir}/rustype-linux
%{_bindir}/rustype-settings
%{_datadir}/applications/dev.namtr.RustypeSettings.desktop
%{_prefix}/lib/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/rustype.conf
%{_datadir}/icons/hicolor/*/apps/*
%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev/
%{_mandir}/man1/*.1*

%changelog
* Mon Nov 09 2026 Nam Tran <support@namtr.dev> - 0.2.1-1
- Fix a GNOME Shell crash in VTE terminals on GNOME 45 to 48.
- Settings, installer, Telex and Windows fixes.

* Mon Nov 09 2026 Nam Tran <support@namtr.dev> - 0.2.0-1
- Add Ctrl+Shift toggle, free tone order, Quick Telex and Simple Telex.

* Mon Nov 09 2026 Nam Tran <support@namtr.dev> - 0.1.1-1
- Replace the IBus engine with rustype-linux.
- Fix VNI spelling of ươ after h, kh and th.

* Sat Nov 07 2026 Nam Tran <support@namtr.dev> - 0.1.0-1
- Initial release for Fedora and RPM distributions.
