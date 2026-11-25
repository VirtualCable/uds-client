# The CA bundle comes from a digest-pinned image. The snapshot is reached over HTTPS, so TLS must work before any
# package is installed, and nothing is fetched from the live archive.
FROM alpine@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf AS ca-bundle

FROM ubuntu:26.04@sha256:19fcd1bf268f02b60aca3bd30bd8a6d9a11759a430e32921833e65f16d87348a
COPY --from=ca-bundle /etc/ssl/certs/ca-certificates.crt /usr/local/share/snapshot-ca.crt
RUN printf '%s\n' 'Acquire::https::CaInfo "/usr/local/share/snapshot-ca.crt";' > /etc/apt/apt.conf.d/80snapshot-tls

ENV DEBIAN_FRONTEND=noninteractive

# Resolve packages from a fixed archive snapshot so clean builds get the same versions.
RUN printf '%s\n' \
  'Types: deb' \
  'URIs: https://snapshot.ubuntu.com/ubuntu/20261123T000000Z' \
  'Suites: resolute resolute-updates resolute-security' \
  'Components: main universe' \
  'Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg' \
  > /etc/apt/sources.list.d/ubuntu.sources

# Install dependencies, for building, for freerdp and for packaging
RUN apt-get update && apt-get install -y \
  ca-certificates curl build-essential pkg-config cmake nasm git libssl-dev libgl1-mesa-dev libglu1-mesa-dev \
  libxft-dev libxext-dev libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libglib2.0-dev libgio-2.0-dev \
  libcairo2-dev libasound2-dev libpulse-dev libclang-dev \
  ninja-build libturbojpeg0-dev libusb-1.0-0-dev \
  libavcodec-dev libavutil-dev libswscale-dev libavformat-dev libswresample-dev \
  libopenh264-dev libkrb5-dev libcups2-dev libfuse3-dev \
  libpcsclite-dev libvpx-dev \
  devscripts debhelper \
  && rm -rf /var/lib/apt/lists/*

ENV FREERDP_ROOT="/usr/local"
# cmake 4 rejects the vendored opus cmake_minimum_required without the policy floor
ENV CMAKE_POLICY_VERSION_MINIMUM="3.5"

ARG FREERDP_COMMIT=3f6d7cb1f8973cc84c66b258a9a61c4e2b2f30a6
WORKDIR /freerdp-build/FreeRDP
RUN git init . \
  && git remote add origin https://github.com/FreeRDP/FreeRDP.git \
  && git fetch --depth 1 origin "${FREERDP_COMMIT}" \
  && git checkout --detach FETCH_HEAD \
  && test "$(git rev-parse HEAD)" = "${FREERDP_COMMIT}"
RUN cmake -Bbuild -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=${FREERDP_ROOT} \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DWITH_CLIENT_COMMON=ON \
  -DWITH_X11=OFF \
  -DWITH_XKBFILE=OFF \
  -DWITH_XCURSOR=OFF \
  -DWITH_XEXT=OFF \
  -DWITH_XRANDR=OFF \
  -DWITH_XRENDER=OFF \
  -DWITH_XFIXES=OFF \
  -DWITH_WAYLAND=OFF \
  -DWITH_OPENGL=OFF \
  -DWITH_CAIRO=OFF \
  -DWITH_DIRECTFB=OFF \
  -DWITH_VAAPI=OFF \
  -DWITH_VAAPI_H264_ENCODING=OFF \
  -DWITH_GDI=ON \
  -DWITH_INTERNAL_MD4=ON \
  -DWITH_INTERNAL_MD5=ON \
  -DWITH_INTERNAL_RC4=ON \
  -DWITH_SDL=OFF \
  -DWITH_SERVER=OFF \
  -DWITH_SHADOW=OFF \
  -DBUILD_TESTING=OFF \
  -DWITH_MANPAGES=OFF \
  -DWITH_CHANNELS=ON \
  -DWITH_CLIENT_CHANNELS=ON \
  -DWITH_RDPSND=ON \
  -DWITH_AUDIN=ON \
  -DWITH_PULSE=ON \
  -DWITH_ALSA=ON \
  -DWITH_DSP_FFMPEG=ON \
  -DWITH_CLIENT_SDL=OFF \
  -DWITH_LIBUSB=ON \
  -DWITH_URBDRC=ON \
  -DWITH_AAD=ON \
  -DWITH_WINPR_JSON=ON \
  -DWITH_CLIENT_WINDOWS=OFF \
  -DWITH_VERBOSE_WINPR_ASSERT=OFF \
  -DWITH_OPENH264=ON \
  -DWITH_GFX_H264=ON \
  -DWITH_RDPGFX=ON \
  -DWITH_DYNVC=ON \
  -DWITH_DEBUG_ALL=OFF \
  -DWITH_DEBUG_CAPABILITIES=OFF \
  -DWITH_DEBUG_CERTIFICATE=OFF \
  -DWITH_DEBUG_CHANNELS=OFF \
  -DWITH_DEBUG_CLIPRDR=OFF \
  -DWITH_DEBUG_CODECS=OFF \
  -DWITH_DEBUG_DVC=OFF \
  -DWITH_DEBUG_KBD=OFF \
  -DWITH_DEBUG_LICENSE=OFF \
  -DWITH_DEBUG_MUTEX=OFF \
  -DWITH_DEBUG_NEGO=OFF \
  -DWITH_DEBUG_NLA=OFF \
  -DWITH_DEBUG_NTLM=OFF \
  -DWITH_DEBUG_RAIL=OFF \
  -DWITH_DEBUG_RDP=OFF \
  -DWITH_DEBUG_RDPDR=OFF \
  -DWITH_DEBUG_RDPEI=OFF \
  -DWITH_DEBUG_RDPGFX=OFF \
  -DWITH_DEBUG_REDIR=OFF \
  -DWITH_DEBUG_RFX=OFF \
  -DWITH_DEBUG_RINGBUFFER=OFF \
  -DWITH_DEBUG_SCARD=OFF \
  -DWITH_DEBUG_SDL_EVENTS=OFF \
  -DWITH_DEBUG_SDL_KBD_EVENTS=OFF \
  -DWITH_DEBUG_SND=OFF \
  -DWITH_DEBUG_SVC=OFF \
  -DWITH_DEBUG_THREADS=OFF \
  -DWITH_DEBUG_TIMEZONE=OFF \
  -DWITH_DEBUG_TRANSPORT=OFF \
  -DWITH_DEBUG_TSG=OFF \
  -DWITH_DEBUG_URBDRC=OFF \
  -DWITH_DEBUG_WND=OFF \
  -DWITH_DEBUG_X11=OFF \
  -DWITH_DEBUG_X11_LOCAL_MOVESIZE=OFF \
  -DWITH_DEBUG_XV=OFF \
  && cmake --build build --parallel \
  && cmake --install build


ARG RUSTUP_VERSION=1.29.3
ARG RUSTUP_INIT_SHA256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55
ARG RUST_TOOLCHAIN=1.100.0
RUN curl -sSfL -o /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
  && echo "${RUSTUP_INIT_SHA256}  /tmp/rustup-init" | sha256sum -c - \
  && chmod +x /tmp/rustup-init \
  && /tmp/rustup-init -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" \
  && rm -f /tmp/rustup-init
ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /
