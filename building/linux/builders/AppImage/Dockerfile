FROM debian:trixie

# Install dependencies for building and for appimage-builder
RUN apt-get update && apt-get install -y \
  curl build-essential pkg-config cmake libssl-dev libgl1-mesa-dev libglu1-mesa-dev \
  libxft-dev libxext-dev libxinerama-dev libxcursor-dev libxfixes-dev \
  libpango1.0-dev libglib2.0-dev libgio-2.0-dev \
  libcairo2-dev freerdp3-dev libasound2-dev libpulse-dev libclang-dev \
  libavcodec-dev libavutil-dev libswscale-dev libavformat-dev libswresample-dev \ 
  libkrb5-dev libopenh264-dev libcups2-dev libfuse3-dev libusb-1.0.0-dev \
  libxss-dev libgtk-3-bin \
  git python3-pip python3-setuptools wget squashfs-tools zsync \
  fuse libfuse2t64 gnupg gpg \
  && rm -rf /var/lib/apt/lists/*

# Set environment variable to allow AppImages to run in Docker without FUSE
ENV APPIMAGE_EXTRACT_AND_RUN=1

# Copy local apt-key to satisfy appimage-builder 1.1.0 on Trixie
COPY apt-key /usr/local/bin/apt-key
RUN chmod +x /usr/local/bin/apt-key

# Install appimage-builder 1.1.0
RUN pip3 install --break-system-packages appimage-builder==1.1.0

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Script to handle the build process
COPY build-appimage.sh /usr/local/bin/build-appimage.sh
RUN chmod +x /usr/local/bin/build-appimage.sh

WORKDIR /crate
