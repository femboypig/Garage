# Building Garage for Windows

This guide provides instructions for building Garage from source on Windows.

## Prerequisites

### 1. Build Tools for Visual Studio

You need the MSVC compiler tools. Install the [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/) and select the **Desktop development with C++** workload.

### 2. Install Rust

Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs).

## Building and Running

Open a Command Prompt or PowerShell window:

```cmd
git clone https://github.com/femboypig/Garage.git
cd Garage
cargo run --release
```

Garage utilizes DX12 or Vulkan via `wgpu` on Windows.
