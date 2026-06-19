# Building Garage for Linux

This guide provides instructions for building Garage from source on Linux.

## Prerequisites

Garage requires a GPU with Vulkan or OpenGL support, along with development packages for windowing and keyboard input.

### Ubuntu / Debian

Install the required packages using `apt`:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  libx11-dev \
  libxcb1-dev \
  libxkbcommon-dev \
  libwayland-dev \
  pkg-config \
  libfreetype6-dev \
  libfontconfig1-dev
```

### Fedora / RHEL

```bash
sudo dnf groupinstall "Development Tools"
sudo dnf install \
  libX11-devel \
  libxcb-devel \
  libxkbcommon-devel \
  wayland-devel \
  pkg-config \
  freetype-devel \
  fontconfig-devel
```

### Arch Linux

```bash
sudo pacman -Syu base-devel libx11 libxcb libxkbcommon wayland pkgconf freetype2 fontconfig
```

### Install Rust

Install the Rust toolchain:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building and Running

Clone the repository and compile the binary:

```bash
git clone https://github.com/femboypig/Garage.git
cd Garage
cargo run --release
```

Garage will default to Vulkan on Linux systems. If Vulkan is unavailable, it will automatically fall back to OpenGL.
