#!/bin/bash

set -e

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$SCRIPT_DIR"

APK_DIR="../../jni-fuzz/top-*/apks/"

# List of apps
APPS=(
  "com.google.android.apps.dynamite"
  "com.whatsapp"
  "org.telegram.messenger"
  "us.zoom.videomeetings"
  "com.paypal.android.p2pmobile"

  "com.teacapps.barcodescanner"
  "com.microsoft.office.outlook"
  "com.microsoft.office.officehubrow"

  "io.opensea"
  "app.token_maker.nft"
)


pushd ../apk_analyzer

BASE_PORT=5580
index=0

for app in "${APPS[@]}"; do
  echo "Installing $app"

  # Install the app
  ANDROID_SERIAL="localhost:$((BASE_PORT + index))" adb install-multiple -g $APK_DIR/$app/*.apk

  index=$((index+1))
done
