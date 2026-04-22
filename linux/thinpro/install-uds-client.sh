#!/bin/sh

echo "Installing UDSClient and UDSRDP"

# unlocks so we can write on TC
fsunlock

# TC hast /bin as a symlink to /usr/bin, so we can copy the client there

cp UDSClient /usr/bin/udsclient
chmod 755 /usr/bin/udsclient
# RDP Script for UDSClient. Launchs udsclient using the "Template_UDS" profile

cp udsrdp /usr/bin
chmod 755 /usr/bin/udsrdp

# Crate if not exists and copy template for UDS connections
if [ ! -d /usr/share/uds ]; then
    mkdir /usr/share/uds
fi

cp Template_UDS.xml /usr/share/uds/Template_UDS.xml
chmod 644 /usr/share/uds/Template_UDS.xml

mclient import /usr/share/uds/Template_UDS.xml
mclient commit

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
