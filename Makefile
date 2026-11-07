# Makefile for Rustype
PREFIX ?= /usr
DESTDIR ?=

CARGO ?= cargo
CARGO_FLAGS ?= --release --locked

.PHONY: all build test check fmt clippy deb install uninstall clean help

all: build

help:
	@echo "Các lệnh khả dụng:"
	@echo "  make build       - Biên dịch release tất cả các crate"
	@echo "  make test        - Chạy toàn bộ unit test và golden test"
	@echo "  make check       - Kiểm tra fmt, clippy và test"
	@echo "  make deb         - Tạo gói Debian (.deb) trong target/packages/"
	@echo "  make install     - Cài đặt vào hệ thống theo PREFIX (mặc định /usr)"
	@echo "  make uninstall   - Gỡ cài đặt khỏi hệ thống theo PREFIX"
	@echo "  make clean       - Dọn dẹp thư mục build target/"

build:
	$(CARGO) build $(CARGO_FLAGS)

test:
	$(CARGO) test $(CARGO_FLAGS)

check: fmt clippy test

fmt:
	$(CARGO) fmt --all --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

deb: build
	./packaging/build-deb.sh

install: build
	install -Dm755 target/release/ibus-engine-rustype $(DESTDIR)$(PREFIX)/libexec/ibus-engine-rustype
	install -Dm755 tools/rustype-settings $(DESTDIR)$(PREFIX)/bin/rustype-settings
	install -Dm644 tools/rustype-settings.desktop $(DESTDIR)$(PREFIX)/share/applications/rustype-settings.desktop
	mkdir -p $(DESTDIR)$(PREFIX)/share/ibus/component
	target/release/ibus-engine-rustype --xml > $(DESTDIR)$(PREFIX)/share/ibus/component/rustype.xml
	for s in 16 32 48 64 128 256 512; do \
		install -Dm644 assets/icons/$${s}x$${s}/rustype.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/$${s}x$${s}/apps/rustype.png; \
	done
	install -Dm644 assets/icons/scalable/rustype.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/rustype.svg
	mkdir -p $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev
	cp -a gnome-shell-extension/rustype-focus@namtr.dev/* $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev/
	install -Dm644 docs/man/ibus-engine-rustype.1 $(DESTDIR)$(PREFIX)/share/man/man1/ibus-engine-rustype.1
	install -Dm644 docs/man/rustype-settings.1 $(DESTDIR)$(PREFIX)/share/man/man1/rustype-settings.1

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/libexec/ibus-engine-rustype
	rm -f $(DESTDIR)$(PREFIX)/bin/rustype-settings
	rm -f $(DESTDIR)$(PREFIX)/share/applications/rustype-settings.desktop
	rm -f $(DESTDIR)$(PREFIX)/share/ibus/component/rustype.xml
	rm -rf $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/*/apps/rustype.png
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/rustype.svg
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/ibus-engine-rustype.1
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/rustype-settings.1

clean:
	$(CARGO) clean
	rm -rf target/deb-staging target/packages
