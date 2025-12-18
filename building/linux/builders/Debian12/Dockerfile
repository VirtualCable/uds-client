FROM debian:bookworm

RUN apt-get update && apt-get install -y \
  curl build-essential pkg-config cmake libssl-dev libgl1-mesa-dev libglu1-mesa-dev \
  libxft-dev libxext-dev libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libglib2.0-dev libgio-2.0-dev \
  libcairo2-dev libasound2-dev libpulse-dev libclang-dev git \
  ninja-build cmake libturbojpeg0-dev libusb-1.0.0-dev \
  libavcodec-dev libavutil-dev libswscale-dev libavformat-dev libswresample-dev \
  libopenh264-dev libkrb5-dev libcups2-dev libfuse3-dev \
  && rm -rf /var/lib/apt/lists/*

ENV FREERDP_ROOT="/usr/local"

WORKDIR /freerdp-build
RUN git clone https://github.com/FreeRDP/FreeRDP.git
WORKDIR /freerdp-build/FreeRDP
RUN cmake -Bbuild -GNinja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=${FREERDP_ROOT} \
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


RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /
