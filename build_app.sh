#!/bin/bash
# Script to package Garage into a native macOS App Bundle (.app) with a custom icon.

set -e

APP_NAME="Garage"
APP_DIR="${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MAC_OS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
LOGO_PATH="/Users/mac/Downloads/Garage-Logo.png"

echo "Building Rust binary..."
cargo build --release

echo "Creating App Bundle directory structure..."
mkdir -p "${MAC_OS_DIR}"
mkdir -p "${RESOURCES_DIR}"

echo "Copying binary..."
cp "target/release/${APP_NAME}" "${MAC_OS_DIR}/${APP_NAME}"

echo "Generating Info.plist..."
cat <<EOF > "${CONTENTS_DIR}/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleIdentifier</key>
    <string>com.femboypig.garage</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.12</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

if [ -f "${LOGO_PATH}" ]; then
    echo "Creating macOS .icns file from PNG using native sips and iconutil..."
    ICONSET_DIR="icon.iconset"
    mkdir -p "${ICONSET_DIR}"

    # Generate all required macOS icon sizes
    sips -z 16 16     "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_16x16.png" > /dev/null 2>&1
    sips -z 32 32     "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_16x16@2x.png" > /dev/null 2>&1
    sips -z 32 32     "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_32x32.png" > /dev/null 2>&1
    sips -z 64 64     "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_32x32@2x.png" > /dev/null 2>&1
    sips -z 128 128   "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_128x128.png" > /dev/null 2>&1
    sips -z 256 256   "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_128x128@2x.png" > /dev/null 2>&1
    sips -z 256 256   "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_256x256.png" > /dev/null 2>&1
    sips -z 512 512   "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_256x256@2x.png" > /dev/null 2>&1
    sips -z 512 512   "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_512x512.png" > /dev/null 2>&1
    sips -z 1024 1024 "${LOGO_PATH}" --out "${ICONSET_DIR}/icon_512x512@2x.png" > /dev/null 2>&1

    iconutil -c icns "${ICONSET_DIR}" -o "${RESOURCES_DIR}/icon.icns"
    rm -rf "${ICONSET_DIR}"
    echo "Custom icon generated and embedded successfully."
else
    echo "Warning: Logo not found at ${LOGO_PATH}, skipping icon generation."
fi

# Touch the app bundle folder to force Finder to refresh its cache
touch "${APP_DIR}"

echo "Success! macOS App Bundle created at: $(pwd)/${APP_DIR}"
EOF
