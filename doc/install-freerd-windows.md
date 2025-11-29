# 📜 Guía de compilación de FreeRDP en Windows con vcpkg

## 1️⃣ Clonar el repositorio de FreeRDP

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
  

cmake -B build `
  -DCMAKE_TOOLCHAIN_FILE="Z:/dev/vcpkg/scripts/buildsystems/vcpkg.cmake" `
  -DVCPKG_TARGET_TRIPLET=x64-windows `
  -DCMAKE_MODULE_PATH="Z:/dev/vcpkg/installed/x64-windows/share/ffmpeg" `
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
  -DWITH_LIBUSB=ON `
  -DWITH_URBDRC=ON `
  -DWITH_AAD=ON `
  -DWITH_WINPR_JSON=ON `
  -DWITH_CLIENT_WINDOWS=OFF `
  -DWITH_VERBOSE_WINPR_ASSERT=OFF `
  -DWITH_OPENH264=ON `
  -DWITH_LAME=ON `
  -DLIBUSB_1_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/libusb-1.0" `
  -DLIBUSB_1_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/libusb-1.0.lib" `
  -DSDL2_TTF_INCLUDE_DIR="Z:/dev/vcpkg/installed/x64-windows/include/SDL2" `
  -DSDL2_TTF_LIBRARY="Z:/dev/vcpkg/installed/x64-windows/lib/SDL2_ttf.lib"
  -DWITH_RDPGFX=ON `
  -DWITH_DYNVC=ON
```

## Para debug:
```powershell
cmake -B build `
  -DCMAKE_TOOLCHAIN_FILE="Z:/dev/vcpkg/scripts/buildsystems/vcpkg.cmake" `
  -DVCPKG_TARGET_TRIPLET=x64-windows `
  -DCMAKE_MODULE_PATH="Z:/dev/vcpkg/installed/x64-windows/share/ffmpeg" `
  -DWITH_CLIENT=ON `
  -DWITH_FFMPEG=ON `
  -DWITH_SDL=ON `
  -DWITH_SERVER=OFF `
  -DWITH_SHADOW=OFF `
  -DBUILD_TESTING=OFF `
  -DWITH_MANPAGES=OFF `
  -DWITH_CHANNELS=ON `
  -DWITH_RDPSND=ON `
  -DWITH_WINMM=ON `
  -DWITH_AUDIN=ON `
  -DWITH_CLIENT_SDL=ON `
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
  -DWITH_LAME=ON `
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


## 2️⃣ Compilar FreeRDP

```powershell
cmake --build build --config Release

cmake --install .\build\ --config Release --prefix z:/dev/freerdp
```

--- WORKING ---

# 📜 Guía de compilación de FreeRDP en Windows con vcpkg

## 1️⃣ Clonar el repositorio de FreeRDP

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

## 2️⃣ Compilar FreeRDP

```powershell
cmake --build build --config Release

cmake --install .\build\ --config Release --prefix z:/dev/freerdp
```

