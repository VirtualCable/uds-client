{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage {
  pname = "%%PNAME%%";
  version = "%%VERSION%%";

  # Ignore the 'target' and 'building' directories so Nix doesn't copy them to the store
  src = pkgs.lib.cleanSourceWith {
    src = ./.;
    filter = path: type:
      let baseName = baseNameOf path;
      in !(baseName == "target" || baseName == "building" || baseName == ".git");
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    cmake
    rustPlatform.bindgenHook
  ];

  buildInputs = with pkgs; [
    openssl
    mesa
    libGLU
    xorg.libXft
    xorg.libXext
    xorg.libXinerama
    xorg.libXcursor
    xorg.libXfixes
    pango
    glib
    cairo
    freerdp3
    alsa-lib
    libpulseaudio
    libclang
    ffmpeg_6
    krb5
    openh264
    cups
    fuse3
    libusb1
  ];

  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
}
