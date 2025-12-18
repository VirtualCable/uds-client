FROM fedora:41

RUN dnf install -y \
    curl gcc make pkgconf-pkg-config cmake openssl-devel mesa-libGL-devel mesa-libGLU-devel \
    libXft-devel libXext-devel libXinerama-devel libXcursor-devel libXfixes-devel \
    pango-devel cairo-devel glib2-devel gcc-c++ libstdc++-devel libstdc++-static \
    && dnf clean all

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"
