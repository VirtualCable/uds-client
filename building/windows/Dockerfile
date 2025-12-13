# Base image: Windows Server Core
FROM mcr.microsoft.com/windows/servercore:ltsc2022
SHELL ["powershell","-NoProfile","-Command"]

# Install Chocolatey for easier dependency management
RUN Set-ExecutionPolicy Bypass -Scope Process -Force; \
    [System.Net.ServicePointManager]::SecurityProtocol = 'Tls12'; \
    iex ((New-Object System.Net.WebClient).DownloadString('https://chocolatey.org/install.ps1'))

# Git, cmake, ninja & LLVM
RUN choco install -y git ; \
    choco install -y cmake --installargs 'ADD_CMAKE_TO_PATH=System' ; \
    choco install -y ninja ; \
    choco install -y llvm ;

# Visual Studio Build Tools 2022
RUN choco install -y visualstudio2022buildtools \
    --package-parameters "'--installPath C:\BuildTools --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --wait --norestart'"

# Define environment variables for Rust, vcpkg, and LLVM. Also define root for FreeRDP build installation
RUN $old=[Environment]::GetEnvironmentVariable('PATH','Machine'); \
    $new=$old+';C:\rust\cargo\bin;C:\vcpkg;C:\Program Files\LLVM\bin'; \
    [Environment]::SetEnvironmentVariable('PATH',$new,'Machine'); \
    [Environment]::SetEnvironmentVariable('CARGO_HOME','C:\rust\cargo','Machine'); \
    [Environment]::SetEnvironmentVariable('RUSTUP_HOME','C:\rust\rustup','Machine'); \
    [Environment]::SetEnvironmentVariable('VCPKG_ROOT','C:\vcpkg','Machine'); \
    [Environment]::SetEnvironmentVariable('FREERDP_ROOT','C:\dev\freerdp','Machine'); \
    [Environment]::SetEnvironmentVariable('LIBCLANG_PATH','C:\Program Files\LLVM\bin','Machine');

# install Rust
RUN Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe; \
    Start-Process -FilePath .\rustup-init.exe -ArgumentList "'-y --default-toolchain stable-x86_64-pc-windows-msvc'" -Wait; \
    Remove-Item rustup-init.exe;

# vcpkg
RUN git clone https://github.com/microsoft/vcpkg $env:VCPKG_ROOT ; \
    & "$env:VCPKG_ROOT\bootstrap-vcpkg.bat"

RUN C:\vcpkg\vcpkg install openssl zlib libjpeg-turbo ffmpeg cjson openh264 libusb

# Clone freerdp and buiild using comp-release.ps1 file
RUN mkdir C:\dev ; \
    git clone https://github.com/FreeRDP/FreeRDP.git C:\dev\FreeRDP.git ; \
    Set-Location C:\dev\FreeRDP.git ; \
    cmake -B build \
    -DCMAKE_TOOLCHAIN_FILE="c:/vcpkg/scripts/buildsystems/vcpkg.cmake" \
    -DVCPKG_TARGET_TRIPLET=x64-windows \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_MODULE_PATH="c:/vcpkg/installed/x64-windows/share/ffmpeg" \
    -DWITH_CLIENT_COMMON=ON \
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
    -DWITH_WINMM=ON \
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
    -DLIBUSB_1_INCLUDE_DIR="c:/vcpkg/installed/x64-windows/include/libusb-1.0" \
    -DLIBUSB_1_LIBRARY="c:/vcpkg/installed/x64-windows/lib/libusb-1.0.lib" \
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
    -DWITH_DEBUG_XV=OFF ; \
    cmake --build build --config Release --parallel 8; \
    cmake --install .\build\ --config Release --prefix $env:FREERDP_ROOT
