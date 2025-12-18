#!/bin/bash

echo "Registering udsactor-service..."
launchctl bootstrap system /Library/LaunchDaemons/org.openuds.udsactor-service.plist

echo "LaunchAgent for udsactor-client will auto-start on login for all users."
