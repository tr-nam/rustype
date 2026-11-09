# GNOME 46 end-to-end image (Ubuntu 24.04). Built from a tar of the working tree (see docker-run.sh),
# so the engine is compiled here against this image's glibc, not copied from the host.

FROM ubuntu:24.04 AS builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl build-essential pkg-config \
 && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH=/root/.cargo/bin:$PATH
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p rustype-linux \
 && install -D target/release/rustype-linux /out/bin/rustype-linux

FROM ubuntu:24.04 AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    gnome-shell mutter gnome-console gnome-terminal \
    gir1.2-gtk-3.0 gir1.2-gtk-4.0 gir1.2-vte-2.91 python3-gi gir1.2-glib-2.0 \
    dbus dbus-user-session libglib2.0-bin pipewire pipewire-bin \
    procps util-linux ca-certificates xwayland ibus-gtk3 \
 && rm -rf /var/lib/apt/lists/*
# Xwayland (case h) makes its sockets in /tmp/.X11-unix: as on a normal system, root-owned and mode 1777.
RUN mkdir -p /tmp/.X11-unix && chmod 1777 /tmp/.X11-unix
COPY --from=builder /out/bin/rustype-linux /opt/rustype/bin/rustype-linux
COPY tests/e2e /opt/rustype/tests/e2e
RUN useradd --create-home --uid 1001 e2e \
 && chmod -R a+rX /opt/rustype
USER e2e
WORKDIR /home/e2e
ENTRYPOINT ["/opt/rustype/tests/e2e/run-all.sh", "/opt/rustype/bin/rustype-linux"]
