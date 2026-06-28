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
    echo "Processing icon to match macOS Big Sur+ rounded squircle standard..."
    
    # Write inline Swift helper to process the image using CoreGraphics/AppKit
    cat <<'EOF' > IconProcessor.swift
import AppKit
import CoreGraphics

let args = CommandLine.arguments
guard args.count >= 3 else {
    print("Usage: icon_processor <input_png> <output_png>")
    exit(1)
}

let inputPath = args[1]
let outputPath = args[2]

guard let rawImage = NSImage(contentsOfFile: inputPath) else {
    print("Error: Could not load input image")
    exit(1)
}

let targetSize = NSSize(width: 1024, height: 1024)
let newImage = NSImage(size: targetSize)

newImage.lockFocus()
let context = NSGraphicsContext.current!.cgContext

// Clear background
context.clear(CGRect(x: 0, y: 0, width: 1024, height: 1024))

// Standard macOS Big Sur template layout:
// Center 824x824 squircle inside 1024x1024 canvas (gives ~10% padding so icon doesn't look giant in Dock)
let padding: CGFloat = 100.0
let contentSize: CGFloat = 824.0
let rect = CGRect(x: padding, y: padding, width: contentSize, height: contentSize)
let cornerRadius: CGFloat = 184.0 // Standard macOS Big Sur corner radius

let path = CGPath(roundedRect: rect, cornerWidth: cornerRadius, cornerHeight: cornerRadius, transform: nil)
context.addPath(path)
context.clip()

// Draw the original image centered and scaled to fill the rounded rect
if let tiffData = rawImage.tiffRepresentation, 
   let imageSource = CGImageSourceCreateWithData(tiffData as CFData, nil), 
   let cgImage = CGImageSourceCreateImageAtIndex(imageSource, 0, nil) {
    context.draw(cgImage, in: rect)
} else {
    rawImage.draw(in: rect)
}

newImage.unlockFocus()

if let tiff = newImage.tiffRepresentation, 
   let bitmap = NSBitmapImageRep(data: tiff), 
   let pngData = bitmap.representation(using: .png, properties: [:]) {
    try? pngData.write(to: URL(fileURLWithPath: outputPath))
    print("Successfully processed logo to macOS squircle standards.")
} else {
    print("Error: Could not save output image")
    exit(1)
}
EOF

    # Compile and run Swift helper
    swiftc -O IconProcessor.swift -o icon_processor
    ./icon_processor "${LOGO_PATH}" processed_logo.png
    
    # Cleanup Swift compiler outputs
    rm -f IconProcessor.swift icon_processor

    echo "Creating macOS .icns file from processed PNG using sips and iconutil..."
    ICONSET_DIR="icon.iconset"
    mkdir -p "${ICONSET_DIR}"

    # Generate all required macOS icon sizes from processed logo
    PROCESSED_LOGO="processed_logo.png"
    sips -z 16 16     "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_16x16.png" > /dev/null 2>&1
    sips -z 32 32     "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_16x16@2x.png" > /dev/null 2>&1
    sips -z 32 32     "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_32x32.png" > /dev/null 2>&1
    sips -z 64 64     "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_32x32@2x.png" > /dev/null 2>&1
    sips -z 128 128   "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_128x128.png" > /dev/null 2>&1
    sips -z 256 256   "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_128x128@2x.png" > /dev/null 2>&1
    sips -z 256 256   "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_256x256.png" > /dev/null 2>&1
    sips -z 512 512   "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_256x256@2x.png" > /dev/null 2>&1
    sips -z 512 512   "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_512x512.png" > /dev/null 2>&1
    sips -z 1024 1024 "${PROCESSED_LOGO}" --out "${ICONSET_DIR}/icon_512x512@2x.png" > /dev/null 2>&1

    iconutil -c icns "${ICONSET_DIR}" -o "${RESOURCES_DIR}/icon.icns"
    
    # Cleanup temporary PNGs
    rm -rf "${ICONSET_DIR}"
    rm -f "${PROCESSED_LOGO}"
    
    echo "Custom squircle icon generated and embedded successfully."
else
    echo "Warning: Logo not found at ${LOGO_PATH}, skipping icon generation."
fi

# Touch the app bundle folder to force Finder to refresh its cache
touch "${APP_DIR}"

echo "Success! macOS App Bundle created at: $(pwd)/${APP_DIR}"
