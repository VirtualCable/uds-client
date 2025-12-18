UDSActor Installation Notes
===========================

Installed Components:
- udsactor-client: LaunchAgent for graphical sessions, auto-starts on user login.
- udsactor-service: LaunchDaemon, runs as root, auto-starts at system boot.
- udsactor-config, gui-helper: supporting binaries.
- udsactor-uninstall: uninstall script located in /usr/local/bin

Service Behavior:
- udsactor-service will restart automatically if it exits with an error.
- If it exits cleanly (exit code 0), it will not restart.

Client Behavior:
- udsactor-client launches once per graphical login.
- It does not restart automatically if closed or crashes.

Manual Control:
- To start the service manually: sudo launchctl start org.openuds.udsactor-service
- To stop the service manually: sudo launchctl stop org.openuds.udsactor-service

Uninstallation:
Run the following command:
    sudo /usr/local/bin/udsactor-uninstall

This will stop and unregister both components, remove all binaries and plist files.

