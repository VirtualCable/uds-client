#!/bin/bash

echo "Stopping and unloading services..."
launchctl bootout system /Library/LaunchDaemons/org.openuds.udsactor-service.plist 2>/dev/null
launchctl bootout gui/$(id -u) /Library/LaunchAgents/org.openuds.udsactor-client.plist 2>/dev/null

echo "Removing plist files..."
rm -f /Library/LaunchDaemons/org.openuds.udsactor-service.plist
rm -f /Library/LaunchAgents/org.openuds.udsactor-client.plist

echo "Removing binaries..."
rm -f /usr/local/bin/udsactor-client
rm -f /usr/local/bin/udsactor-service
rm -f /usr/local/bin/udsactor-config
rm -f /usr/local/bin/gui-helper
rm -f /usr/local/bin/udsactor-uninstall

echo "Removing documentation..."
rm -f /usr/local/share/doc/udsactor/README.txt
rm -f /usr/local/share/doc/udsactor/license.txt
rmdir /usr/local/share/doc/udsactor 2>/dev/null || true
rmdir /usr/local/share/doc 2>/dev/null || true

echo "UDSActor uninstalled."
