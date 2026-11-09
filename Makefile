# Makefile for Rustype (rustype-linux, IBus on GNOME)
PREFIX ?= /usr
DESTDIR ?=

CARGO ?= cargo
CARGO_FLAGS ?= --release --locked

UNIT = org.freedesktop.IBus.session.GNOME.service
DROPIN_DIR = $(PREFIX)/lib/systemd/user/$(UNIT).d

.PHONY: all build test test-integration check fmt clippy deb sandbox install uninstall clean help

all: build

help:
	@echo "Các lệnh khả dụng:"
	@echo "  make build       - Biên dịch release rustype-linux"
	@echo "  make test        - Chạy toàn bộ unit test và golden test"
	@echo "  make test-integration - Chạy kiểm thử tích hợp: install.sh, uninstall.sh, gói .deb (trong sandbox)"
	@echo "  make check       - Kiểm tra fmt, clippy và test"
	@echo "  make sandbox     - Chạy thử rustype-linux trên D-Bus riêng (không đụng IBus thật)"
	@echo "  make deb         - Tạo gói Debian (.deb) trong target/packages/"
	@echo "  make install     - Cài đặt vào hệ thống theo PREFIX (mặc định /usr)"
	@echo "  make uninstall   - Gỡ cài đặt khỏi hệ thống theo PREFIX"
	@echo "  make clean       - Dọn dẹp thư mục build target/"

build:
	$(CARGO) build $(CARGO_FLAGS) -p rustype-linux

test:
	$(CARGO) test $(CARGO_FLAGS)

test-integration: build
	RUSTYPE_LINUX_BIN=$(CURDIR)/target/release/rustype-linux tests/run_all.sh

check: fmt clippy test

fmt:
	$(CARGO) fmt --all --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

sandbox: build
	tools/rustype_linux_sandbox.sh $(CURDIR)/target/release/rustype-linux

deb: build
	./packaging/build-deb.sh

install: build
	install -Dm755 target/release/rustype-linux $(DESTDIR)$(PREFIX)/libexec/rustype-linux
	mkdir -p $(DESTDIR)$(PREFIX)/bin
	ln -sf ../libexec/rustype-linux $(DESTDIR)$(PREFIX)/bin/rustype-linux
	install -Dm755 tools/rustype-settings $(DESTDIR)$(PREFIX)/bin/rustype-settings
	install -Dm644 tools/dev.namtr.RustypeSettings.desktop $(DESTDIR)$(PREFIX)/share/applications/dev.namtr.RustypeSettings.desktop
	for s in 16 32 48 64 128 256 512; do \
		install -Dm644 assets/icons/$${s}x$${s}/rustype.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/$${s}x$${s}/apps/rustype.png; \
		ln -sf rustype.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/$${s}x$${s}/apps/dev.namtr.RustypeSettings.png; \
	done
	install -Dm644 assets/icons/scalable/rustype.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/rustype.svg
	ln -sf rustype.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/dev.namtr.RustypeSettings.svg
	mkdir -p $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev
	cp -a gnome-shell-extension/rustype-focus@namtr.dev/* $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev/
	install -Dm644 docs/man/rustype-linux.1 $(DESTDIR)$(PREFIX)/share/man/man1/rustype-linux.1
	install -Dm644 docs/man/rustype-settings.1 $(DESTDIR)$(PREFIX)/share/man/man1/rustype-settings.1
	mkdir -p $(DESTDIR)$(DROPIN_DIR)
	printf '[Service]\nExecStart=\nExecStart=$(PREFIX)/libexec/rustype-linux --own-session-name\nRestart=on-failure\nRestartSec=1\n' > $(DESTDIR)$(DROPIN_DIR)/rustype.conf
	chmod 644 $(DESTDIR)$(DROPIN_DIR)/rustype.conf

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/libexec/rustype-linux
	rm -f $(DESTDIR)$(PREFIX)/bin/rustype-linux
	rm -f $(DESTDIR)$(PREFIX)/bin/rustype-settings
	rm -f $(DESTDIR)$(PREFIX)/share/applications/dev.namtr.RustypeSettings.desktop
	rm -f $(DESTDIR)$(DROPIN_DIR)/rustype.conf
	-rmdir $(DESTDIR)$(DROPIN_DIR) 2>/dev/null
	rm -rf $(DESTDIR)$(PREFIX)/share/gnome-shell/extensions/rustype-focus@namtr.dev
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/*/apps/rustype.png
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/*/apps/dev.namtr.RustypeSettings.png
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/rustype.svg
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/dev.namtr.RustypeSettings.svg
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/rustype-linux.1
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/rustype-settings.1

clean:
	$(CARGO) clean
	rm -rf target/deb-staging target/packages
