FROM opensuse/leap:15.6

RUN zypper refresh && zypper install -y \
    curl gcc make pkgconf cmake libopenssl-devel \
    Mesa-libGL-devel Mesa-libGLU-devel \
    libXft-devel libXext-devel libXinerama-devel \
    libXcursor-devel libXfixes-devel \
    pango-devel cairo-devel glib2-devel \
    gcc-c++ libstdc++-devel \
    tar gzip bzip2 xz zip unzip \
    && zypper clean

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"
