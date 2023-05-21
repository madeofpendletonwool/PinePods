#!/bin/bash

# Replace these with your actual values
APP_PATH="/path/to/myapplication"
ICON_PATH="/path/to/myapplication/icon.png"
APP_NAME="My Application"
COMMENT="My Application"

cat > ~/.local/share/applications/myapplication.desktop << EOF
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
