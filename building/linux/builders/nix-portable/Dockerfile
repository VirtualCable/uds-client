FROM debian:trixie

# Install basic dependencies to run nix-portable
RUN apt-get update && apt-get install -y \
  curl git xz-utils bzip2 ca-certificates libfuse3-4 \
  && rm -rf /var/lib/apt/lists/*

ENV NP_GIT=/usr/bin/git

# Download nix-portable
RUN curl -L https://github.com/DavHau/nix-portable/releases/latest/download/nix-portable-$(uname -m) -o /usr/local/bin/nix-portable \
    && chmod +x /usr/local/bin/nix-portable

# Precache Rust and main dependencies to speed up the builds
# We use proot here because bwrap fails in restricted docker build environments
RUN NP_RUNTIME=proot nix-portable nix-shell -p cargo rustc pkg-config cmake rustPlatform.bindgenHook openssl mesa libGLU xorg.libXft xorg.libXext xorg.libXinerama xorg.libXcursor xorg.libXfixes pango glib cairo freerdp alsa-lib libpulseaudio libclang ffmpeg_6 krb5 openh264 cups fuse3 libusb1 --run "echo Precached dependencies"

# Copy the build script
COPY build-nix.sh /usr/local/bin/build-nix.sh
RUN chmod +x /usr/local/bin/build-nix.sh

