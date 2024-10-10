#!/bin/sh

# Check for root
if ! [ $(id -u) = 0 ]; then
   echo "This script must be run as root" 
   exit 1
fi

# If -r flag, remove the appimage
if [ "$1" = "-r" ]; then
    echo "Removing UDSClient Portable..."
    rm /usr/bin/UDSClient-0.0.0-x86_64.AppImage
    rm /usr/share/applications/UDSClient.desktop
    update-desktop-database
    echo "Removal process done."
    exit 0
fi

echo "Installing UDSClient Portable..."
cp UDSClient-0.0.0-x86_64.AppImage /usr/bin
chmod 755 /usr/bin/UDSClient-0.0.0-x86_64.AppImage
cp UDSClient.desktop /usr/share/applications
update-desktop-database

echo "Installation process done."
