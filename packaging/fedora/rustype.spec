Name:           rustype
Version:        0.1.0
Release:        1%{?dist}
Summary:        Lightweight Vietnamese input method engine for IBus

License:        MIT
URL:            https://github.com/namtr/rustype
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.70.0

Requires:       ibus
Requires:       python3
Requires:       python3-gobject
Requires:       libadwaita
Requires:       gtk4
Recommends:     gnome-shell

%description
Rustype is an ultra-lightweight, high-performance, and privacy-focused
Vietnamese input method engine designed for IBus on Linux. Written in
pure Rust with zero external GUI dependencies in core, maintaining a
minimal ~2.5 MB RAM footprint. Supports Telex and VNI with modern GTK4
Libadwaita settings.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked

%install
# Binaries
install -Dm755 target/release/ibus-engine-rustype %{buildroot}%{_libexecdir}/ibus-engine-rustype
install -Dm755 tools/rustype-settings %{buildroot}%{_bindir}/rustype-settings

# Desktop entry
install -Dm644 tools/rustype-settings.desktop %{buildroot}%{_datadir}/applications/rustype-settings.desktop

# IBus component XML
mkdir -p %{buildroot}%{_datadir}/ibus/component
cat <<EOF > %{buildroot}%{_datadir}/ibus/component/rustype.xml
<?xml version="1.0" encoding="utf-8"?>
<component>
    <name>org.freedesktop.IBus.Rustype</name>
    <description>Rustype - bộ gõ tiếng Việt Telex &amp; VNI siêu nhẹ</description>
    <exec>%{_libexecdir}/ibus-engine-rustype --ibus</exec>
    <version>%{version}</version>
    <author>Nam Trần</author>
    <license>MIT</license>
    <homepage>https://github.com/namtr/rustype</homepage>
    <textdomain>rustype</textdomain>
    <engines>
        <engine>
            <name>rustype</name>
            <language>vi</language>
            <license>MIT</license>
            <author>Nam Trần</author>
            <icon>rustype</icon>
            <layout>us</layout>
            <longname>Rustype</longname>
            <description>Bộ gõ siêu nhẹ bằng Rust cho Linux</description>
            <rank>99</rank>
            <symbol>R</symbol>
        </engine>
    </engines>
</component>
EOF
chmod 644 %{buildroot}%{_datadir}/ibus/component/rustype.xml

# Icons
for s in 16 32 48 64 128 256 512; do
    install -Dm644 assets/icons/${s}x${s}/rustype.png %{buildroot}%{_datadir}/icons/hicolor/${s}x${s}/apps/rustype.png
done
install -Dm644 assets/icons/scalable/rustype.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/rustype.svg

# GNOME Shell extension
mkdir -p %{buildroot}%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev
cp -a gnome-shell-extension/rustype-focus@namtr.dev/* %{buildroot}%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev/

# Man pages
install -Dm644 docs/man/ibus-engine-rustype.1 %{buildroot}%{_mandir}/man1/ibus-engine-rustype.1
install -Dm644 docs/man/rustype-settings.1 %{buildroot}%{_mandir}/man1/rustype-settings.1

%files
%license LICENSE
%doc README.md CHANGELOG.md docs/PRIVACY.md
%{_libexecdir}/ibus-engine-rustype
%{_bindir}/rustype-settings
%{_datadir}/applications/rustype-settings.desktop
%{_datadir}/ibus/component/rustype.xml
%{_datadir}/icons/hicolor/*/apps/rustype.*
%{_datadir}/gnome-shell/extensions/rustype-focus@namtr.dev/
%{_mandir}/man1/*.1*

%changelog
* Sat Nov 07 2026 Nam Tran <tr.namm29@gmail.com> - 0.1.0-1
- Initial release for Fedora and RPM distributions.
