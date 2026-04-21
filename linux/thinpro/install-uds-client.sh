#!/bin/sh

echo "Installing UDSClient and UDSRDP"

# unlocks so we can write on TC
fsunlock


cp UDSClient /bin/udsclient
chmod 755 /bin/udsclient
# RDP Script for UDSClient. Launchs udsclient using the "Template_UDS" profile

cp udsrdp /usr/bin
chmod 755 /usr/bin/udsrdp

# Copy handlers for firefox
mkdir -p /lib/UDSClient/firefox/ > /dev/null 2>&1
# Copy handlers.json for firefox
cp firefox/handlers.json /lib/UDSClient/firefox/
cp firefox/45-uds /etc/hptc-firefox-mgr/prestart
# copy uds handler for firefox
cp firefox/uds /usr/share/hptc-firefox-mgr/handlers/uds
chmod 755 /usr/share/hptc-firefox-mgr/handlers/uds

# Common part
fslock
