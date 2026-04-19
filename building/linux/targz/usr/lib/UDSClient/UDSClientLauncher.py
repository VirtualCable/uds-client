import sys
import os.path
import subprocess
import typing

from uds.log import logger
import UDSClient

# Basically, this will be the main file for the Mac OS X application
# It will be called from the Info.plist file, and will be responsible for
# launching the UDSClient application, and also for handling the URL
# that will be passed to it.
# In order to do so, we will call "ourselves" with the URL as parameter, and
# in that case, we will process the URL and launch the UDSClient application
# If no url is passed, UDSClientLauncher will be instantiated, waiting for
# the event that will be generated when the application is called with the
# URL as parameter.

from uds.ui import QtCore, QtWidgets, QtGui, Ui_MacLauncher

SCRIPT_NAME = 'UDSClientLauncher'

class UDSClientLauncher(QtWidgets.QApplication):
    path: str
    tunnels: 'typing.List[subprocess.Popen[typing.Any]]'

    def __init__(self, argv: typing.List[str]) -> None:
        super().__init__(argv)
        self.path = os.path.join(os.path.dirname(sys.argv[0]).replace('Resources', 'MacOS'), SCRIPT_NAME)
        self.tunnels = []
        self.lastWindowClosed.connect(self.close_tunnels)

    def clean_tunnels(self) -> None:
        '''
        Removes all finished tunnels from the list
        '''

        def _is_running(p: subprocess.Popen[typing.Any]) -> bool:
            try:
                if p.poll() is None:
                    return True
            except Exception as e:
                logger.debug('Got error polling subprocess: %s', e)
            return False

        # Remove references to finished tunnels, they will be garbage collected
        self.tunnels = [tunnel for tunnel in self.tunnels if _is_running(tunnel)]

    def close_tunnels(self) -> None:
        '''
        Finishes all running tunnels
        '''
        logger.debug('Closing remaining tunnels')
        for tunnel in self.tunnels:
            logger.debug('Checking %s - "%s"', tunnel, tunnel.poll())
            if tunnel.poll() is None:  # Running
                logger.info('Found running tunnel %s, closing it', tunnel.pid)
                tunnel.kill()

    def event(self, evnt: QtCore.QEvent) -> bool:  # pyright: ignore[reportIncompatibleMethodOverride]
        if evnt.type() == QtCore.QEvent.Type.FileOpen:
            fe = typing.cast(QtGui.QFileOpenEvent, evnt)
            logger.debug('Got url: %s', fe.url().url())
            fe.accept()
            logger.debug('Spawning %s', self.path)
            # First, remove all finished tunnel processed from check queue, to keelp things clean
            self.clean_tunnels()
            # And now add a new one, calling self with the url
            self.tunnels.append(subprocess.Popen([self.path, fe.url().url()]))

        return super().event(evnt)


def main(args: typing.List[str]) -> None:
    if len(args) > 1:
        UDSClient.main(args)
    else:
        app = UDSClientLauncher([])  # no args for launcher needed
        window = QtWidgets.QMainWindow()
        Ui_MacLauncher().setupUi(window)  # type: ignore

        window.showMinimized()

        sys.exit(app.exec())


if __name__ == "__main__":
    main(args=sys.argv)
