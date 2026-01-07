FROM debian:trixie

# Install dependencies, for building, for freerdp and for packaging
RUN apt-get update && apt-get install -y \
  curl build-essential pkg-config cmake libssl-dev libgl1-mesa-dev libglu1-mesa-dev \
  libxft-dev libxext-dev libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libglib2.0-dev libgio-2.0-dev \
  libcairo2-dev freerdp3-dev libasound2-dev libpulse-dev libclang-dev \
  libavcodec-dev libavutil-dev libswscale-dev libavformat-dev libswresample-dev \ 
  libkrb5-dev libopenh264-dev libcups2-dev libfuse3-dev libusb-1.0.0-dev \
  devscripts debhelper \
  && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"
