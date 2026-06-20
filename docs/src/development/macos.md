# Building Garage for macOS

This guide provides instructions for building Garage from source on macOS.

## Prerequisites

### 1. Install Command Line Tools

You will need Xcode Command Line Tools to compile C/C++ dependencies:

```bash
xcode-select --install
```

### 2. Install Rust

Install the Rust toolchain via rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Make sure you are using the latest stable toolchain.

## Building and Running

Clone the repository and run the editor:

```bash
git clone https://github.com/femboypig/Garage.git
cd Garage
cargo run --release
```

Garage utilizes Metal via `wgpu` automatically on macOS.