# 📜 Very simple guide to building FreeRDP on Windows with vcpkg

## Clone the FreeRDP repository

```powershell
git clone https://github.com/FreeRDP/FreeRDP.git
cd FreeRDP

vcpkg install `
  zlib:x64-windows `
  openssl:x64-windows `
  ffmpeg:x64-windows `
  libusb:x64-windows `
  sdl3:x64-windows `
  openh264:x64-windows `
  
Values for direct audio support
  -DWITH_ALSA=ON `
  -DWITH_PULSE=ON `
  -DWITH_WINMM=ON `
  -DWITH_MACAUDIO=ON `

Values for audio codecs
  -DWITH_LAME=ON `
  -DWITH_FAAD2=ON `
  -DWITH_FAAC=ON `
  -DWITH_GSM=ON `
  -DWITH_SOXR=ON `
  -DWITH_DSP_FFMPEG=ON

cmake -B build `
  -DCMAKE_TOOLCHAIN_FILE="Z:/dev/vcpkg/scripts/buildsystems/vcpkg.cmake" `
  -DVCPKG_TARGET_TRIPLET=x64-windows `
  -DCMAKE_BUILD_TYPE=Release `
  -DCMAKE_MODULE_PATH="Z:/dev/vcpkg/installed/x64-windows/share/ffmpeg" `
  -DWITH_CLIENT_COMMON=ON `
  -DWITH_INTERNAL_MD4=ON `
  -DWITH_INTERNAL_MD5=ON `
  -DWITH_INTERNAL_RC4=ON `
  -DWITH_SDL=OFF `
  -DWITH_SERVER=OFF `
  -DWITH_SHADOW=OFF `
  -DBUILD_TESTING=OFF `
  -DWITH_MANPAGES=OFF `
  -DWITH_CHANNELS=ON `
  -DWITH_CLIENT_CHANNELS=ON `
  -DWITH_RDPSND=ON `
  -DWITH_AUDIN=ON `
  -DWITH_DSP_FFMPEG=ON `
  -DWITH_CLIENT_SDL=OFF `
  -DWITH_LIBUSB=ON `
  -DWITH_URBDRC=ON `
  -DWITH_AAD=ON `
  -DWITH_WINPR_JSON=ON `
  -DWITH_CLIENT_WINDOWS=OFF `
  -DWITH_VERBOSE_WINPR_ASSERT=OFF `
  -DWITH_OPENH264=ON `
  -DCHANNEL_SERIAL_CLIENT=ON `
  -DLIBUSB_1_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/libusb-1.0" `
  -DLIBUSB_1_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/libusb-1.0.lib" `
  -DWITH_RDPGFX=ON `
  -DWITH_DYNVC=ON `
  -DCHANNEL_TELEMETRY_CLIENT=ON `
  -DCHANNEL_TELEMETRY_SERVER=OFF `
  -DWITH_DEBUG_ALL=OFF `
  -DWITH_DEBUG_CAPABILITIES=OFF `
  -DWITH_DEBUG_CERTIFICATE=OFF `
  -DWITH_DEBUG_CHANNELS=OFF `
  -DWITH_DEBUG_CLIPRDR=OFF `
  -DWITH_DEBUG_CODECS=OFF `
  -DWITH_DEBUG_DVC=OFF `
  -DWITH_DEBUG_KBD=OFF `
  -DWITH_DEBUG_LICENSE=OFF `
  -DWITH_DEBUG_MUTEX=OFF `
  -DWITH_DEBUG_NEGO=OFF `
  -DWITH_DEBUG_NLA=OFF `
  -DWITH_DEBUG_NTLM=OFF `
  -DWITH_DEBUG_RAIL=OFF `
  -DWITH_DEBUG_RDP=OFF `
  -DWITH_DEBUG_RDPDR=OFF `
  -DWITH_DEBUG_RDPEI=OFF `
  -DWITH_DEBUG_RDPGFX=OFF `
  -DWITH_DEBUG_REDIR=OFF `
  -DWITH_DEBUG_RFX=OFF `
  -DWITH_DEBUG_RINGBUFFER=OFF `
  -DWITH_DEBUG_SCARD=OFF `
  -DWITH_DEBUG_SDL_EVENTS=OFF `
  -DWITH_DEBUG_SDL_KBD_EVENTS=OFF `
  -DWITH_DEBUG_SND=OFF `
  -DWITH_DEBUG_SVC=OFF `
  -DWITH_DEBUG_THREADS=OFF `
  -DWITH_DEBUG_TIMEZONE=OFF `
  -DWITH_DEBUG_TRANSPORT=OFF `
  -DWITH_DEBUG_TSG=OFF `
  -DWITH_DEBUG_URBDRC=OFF `
  -DWITH_DEBUG_WND=OFF `
  -DWITH_DEBUG_X11=OFF `
  -DWITH_DEBUG_X11_LOCAL_MOVESIZE=OFF `
  -DWITH_DEBUG_XV=OFF


cmake --build build --config Release --parallel 16
cmake --install .\build\ --config Release --prefix z:/dev/freerdp  
```

# SDL note:
#   -DSDL2_TTF_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/SDL2" `
#   -DSDL2_TTF_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/SDL2_ttf.lib"


## For debug:
```powershell
cmake -B build `
  -DCMAKE_TOOLCHAIN_FILE="Z:/dev/vcpkg/scripts/buildsystems/vcpkg.cmake" `
  -DVCPKG_TARGET_TRIPLET=x64-windows `
  -DCMAKE_MODULE_PATH="Z:/dev/vcpkg/installed/x64-windows/share/ffmpeg" `
  -DWITH_CLIENT_COMMON=ON `
  -DWITH_INTERNAL_MD4=ON `
  -DWITH_INTERNAL_MD5=ON `
  -DWITH_INTERNAL_RC4=ON `
  -DWITH_CLIENT=ON `
  -DWITH_FFMPEG=ON `
  -DWITH_SDL=OFF `
  -DWITH_SERVER=OFF `
  -DWITH_SHADOW=OFF `
  -DBUILD_TESTING=OFF `
  -DWITH_MANPAGES=OFF `
  -DWITH_CHANNELS=ON `
  -DWITH_RDPSND=ON `
  -DWITH_WINMM=ON `
  -DWITH_AUDIN=ON `
  -DWITH_CLIENT_SDL=OFF `
  -DWITH_RDPGFX=ON `
  -DWITH_DYNVC=ON `
  -DWITH_LIBUSB=ON `
  -DWITH_URBDRC=ON `
  -DWITH_SWSCALE=ON `
  -DWITH_AAD=ON `
  -DWITH_WINPR_JSON=ON `
  -DWITH_CLIENT_WINDOWS=ON `
  -DWITH_VERBOSE_WINPR_ASSERT=ON `
  -DWITH_OPENH264=ON `
  -DLIBUSB_1_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/libusb-1.0" `
  -DLIBUSB_1_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/libusb-1.0.lib" `
  -DSDL2_TTF_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/SDL2" `
  -DSDL2_TTF_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/SDL2_ttf.lib" `
  -DWITH_SAMPLE=ON `
  -DCMAKE_BUILD_TYPE=Debug `
  -DWITH_DEBUG_ALL=ON `
  -DWITH_DEBUG_EVENTS=OFF `
  -DWITH_WIN_CONSOLE=ON `
  -DWITH_SANITIZE_ADDRESS=ON

cmake --build build --config Debug --parallel 16

cmake --install .\build\ --config Debug --prefix z:/dev/freerdp

```


## Build FreeRDP

```powershell
cmake --build build --config Release

cmake --install .\build\ --config Release --prefix z:/dev/freerdp
```

--- WORKING (for sdl! for uds client remove all SDL related) ---

# Very simple guide

## Clone the FreeRDP repository

```powershell
git clone https://github.com/FreeRDP/FreeRDP.git
cd FreeRDP

vcpkg install `
  zlib:x64-windows `
  openssl:x64-windows `
  ffmpeg:x64-windows `
  libusb:x64-windows `
  sdl2:x64-windows `
  sdl2-ttf:x64-windows

cmake -B build `
  -DCMAKE_TOOLCHAIN_FILE="Z:/dev/vcpkg/scripts/buildsystems/vcpkg.cmake" `
  -DVCPKG_TARGET_TRIPLET=x64-windows `
  -DCMAKE_MODULE_PATH="Z:/dev/vcpkg/installed/x64-windows/share/ffmpeg" `
  -DWITH_SDL=ON `
  -DWITH_SERVER=OFF `
  -DWITH_SHADOW=OFF `
  -DBUILD_TESTING=OFF `
  -DWITH_MANPAGES=OFF `
  -DWITH_CHANNELS=ON `
  -DWITH_CLIENT_SDL=ON `
  -DWITH_LIBUSB=ON `
  -DWITH_URBDRC=ON `
  -DLIBUSB_1_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/libusb-1.0" `
  -DLIBUSB_1_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/libusb-1.0.lib" `
  -DSDL2_TTF_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/SDL2" `
  -DSDL2_TTF_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/SDL2_ttf.lib"
```

## Build FreeRDP

```powershell
cmake --build build --config Release

cmake --install .\build\ --config Release --prefix z:/dev/freerdp
```

