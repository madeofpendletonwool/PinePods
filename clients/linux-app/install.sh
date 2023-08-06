#!/bin/bash

# Define your local paths
APP_PATH="$HOME/.local/bin"
ICON_PATH="$HOME/.config/pinepods/pinepods-appicon.png"

# Create necessary directories
mkdir -p $APP_PATH
mkdir -p $(dirname $ICON_PATH)

# Download the app and the icon
cp pinepods $APP_PATH
cp pinepods-appicon.png $ICON_PATH

# Set executable permissions on the app
chmod +x $APP_PATH

# Replace these with your actual values
APP_NAME="Pinepods"
COMMENT="A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"

# Create the desktop file
cat > ~/.local/share/applications/pinepods.desktop << EOF
[Desktop Entry]
Version=1.0
Name=$APP_NAME
Comment=$COMMENT
Exec=$APP_PATH/pinepods
Icon=$ICON_PATH
Terminal=false
Type=Application
Categories=Utility;Application;
EOF

# Update the desktop file database
update-desktop-database ~/.local/share/applications

echo "Desktop file created and database updated"

