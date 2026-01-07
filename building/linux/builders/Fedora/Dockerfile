FROM fedora:42

RUN dnf install -y \
    curl gcc gcc-c++ make pkgconf-pkg-config cmake \
    openssl-devel mesa-libGL-devel mesa-libGLU-devel \
    libXft-devel libXext-devel libXinerama-devel libXcursor-devel libXfixes-devel \
    pango-devel cairo-devel glib2-devel \
    freerdp-devel alsa-lib-devel pulseaudio-libs-devel clang-devel \
    ffmpeg-free-devel krb5-devel openh264-devel cups-devel fuse3-devel libusbx-devel \
    rpm-build rpmdevtools strip redhat-rpm-config \
    && dnf clean all

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"
