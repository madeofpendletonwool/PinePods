#!/bin/bash

# Replace these with your actual values
APP_PATH="~/.local/bin/pypods"
ICON_PATH="~/.config/pinepods/pinepods-appicon.png"
APP_NAME="Pinepods"
COMMENT="This is the desktop file for Pinepods. Do not edit this as changes are made automatically upon updates."

cat > ~/.local/share/applications/pinepods.desktop << EOF
[Desktop Entry]
Version=1.0
Name=$APP_NAME
Comment=$COMMENT
Exec=$APP_PATH
Icon=$ICON_PATH
Terminal=false
Type=Application
Categories=Utility;Application;
EOF

# Update the desktop file database
update-desktop-database ~/.local/share/applications

echo "Desktop file created and database updated"
