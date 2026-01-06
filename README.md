# OpenUDS Launcher

✅ **New launcher for OpenUDS** — This repository contains the new OpenUDS launcher implemented in Rust.

## Overview

This project implements a modern launcher for OpenUDS. The launcher code lives in the `launcher` crate (see `crates/launcher`). It provides the client-side application used to start and manage OpenUDS sessions.

## Requirements

- Rust toolchain (stable)
- OpenGL support (required by the GUI)

> ⚠️ On Windows, OpenGL support is usually provided by the GPU driver. If your system lacks adequate OpenGL support, you can install a Windows build of Mesa from:

https://github.com/pal1000/mesa-dist-win/releases

## Build & Run

From the repository root you can build and run the launcher directly with Cargo:

- Build (release):

In the building/... directory, you have the scripts to build the launcher for different platforms
   * Windows version uses Docker to create a consistent build environment. (Docker for Windows is required)
   * Linux version Same. as Windows, but for Linux.
   * macOS version uses a macOS machine with Rust installed.
  
There is some more info about building inside the doc folder (such as how to build without Docker, or register the application in Windows).


