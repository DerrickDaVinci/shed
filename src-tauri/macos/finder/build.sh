#!/bin/sh
# Builds ShedFinder.appex (the Finder extension) for both Mac architectures
# and signs it ad hoc with its sandbox entitlement. Tauri copies it into
# Shed.app/Contents/PlugIns (see bundle.macOS.files in tauri.conf.json).
set -eu
cd "$(dirname "$0")"
out=build/ShedFinder.appex
rm -rf build
mkdir -p "$out/Contents/MacOS"
cp Info.plist "$out/Contents/Info.plist"
clang -fobjc-arc -fmodules -fapplication-extension -arch arm64 -arch x86_64 -mmacosx-version-min=11.0 \
  -framework Cocoa -framework FinderSync -e _NSExtensionMain -O2 \
  -o "$out/Contents/MacOS/ShedFinder" ShedFinder.m
codesign --force --sign - --entitlements ShedFinder.entitlements --options runtime "$out"
