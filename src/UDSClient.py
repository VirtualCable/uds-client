#!/usr/bin/env -S python3 -s
# -*- coding: utf-8 -*-
#
# Copyright (c) 2014-2024 Virtual Cable S.L.U.
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without modification,
# are permitted provided that the following conditions are met:
#
#    * Redistributions of source code must retain the above copyright notice,
#      this list of conditions and the following disclaimer.
#    * Redistributions in binary form must reproduce the above copyright notice,
#      this list of conditions and the following disclaimer in the documentation
#      and/or other materials provided with the distribution.
#    * Neither the name of Virtual Cable S.L.U. nor the names of its contributors
#      may be used to endorse or promote products derived from this software
#      without specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
# AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
# DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

# pyright: reportUnknownMemberType=false
# ,reportUnknownParameterType=false
# ,reportUnknownArgumentType=false
'''
Author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import sys
import os
import platform
import time
import webbrowser
import threading
import typing

from uds.ui import QtCore, QtWidgets, QtGui, QSettings, Ui_MainWindow
from uds.rest import RestApi, RetryException, InvalidVersion

# Just to ensure there are available on runtime
from uds.tunnel import forward as tunnel_forwards  # pyright: ignore[reportUnusedImport]

from uds.log import logger
from uds import consts, tools
from uds import VERSION


class UDSClient(QtWidgets.QMainWindow):
    ticket: str = ''
    scrambler: str = ''
    has_error = False
    animation_timer: typing.Optional[QtCore.QTimer] = None
    animation_value: int = 0
    animation_reversed: bool = False
    api: RestApi

    def __init__(self, api: RestApi, ticket: str, scrambler: str) -> None:
        QtWidgets.QMainWindow.__init__(self)
        self.api = api
        self.ticket = ticket
        self.scrambler = scrambler
        self.setWindowFlags(
            QtCore.Qt.WindowType.FramelessWindowHint | QtCore.Qt.WindowType.WindowStaysOnTopHint
        )

        self.ui = Ui_MainWindow()
        self.ui.setupUi(self)  # type: ignore

        self.ui.progressBar.setValue(0)
        self.ui.cancelButton.clicked.connect(self.on_cancel_pressed)

        self.ui.info.setText('Initializing...')

        screen_geometry = QtGui.QGuiApplication.primaryScreen().geometry()
        mysize = self.geometry()
        hpos = (screen_geometry.width() - mysize.width()) // 2
        vpos = (screen_geometry.height() - mysize.height() - mysize.height()) // 2
        self.move(hpos, vpos)

        self.animation_timer = QtCore.QTimer()
        self.animation_timer.timeout.connect(self.update_anum)
        # QtCore.QObject.connect(self.animTimer, QtCore.SIGNAL('timeout()'), self.updateAnim)

        self.activateWindow()

        self.start_animation()

    def close_window(self) -> None:
        self.close()

    def show_error(self, error: Exception) -> None:
        logger.error('got error: %s', error)
        self.stop_animation()
        # In fact, main window is hidden, so this is not visible... :)
        self.ui.info.setText('UDS Plugin Error')
        self.close_window()
        QtWidgets.QMessageBox.critical(
            None,  # type: ignore
            'UDS Plugin Error',
            '{}'.format(error),
            QtWidgets.QMessageBox.StandardButton.Ok,
        )
        self.has_error = True

    def on_cancel_pressed(self) -> None:
        self.close()

    def update_anum(self) -> None:
        self.animation_value += 2
        if self.animation_value > 99:
            self.animation_reversed = not self.animation_reversed
            self.ui.progressBar.setInvertedAppearance(self.animation_reversed)
            self.animation_value = 0

        self.ui.progressBar.setValue(self.animation_value)

    def start_animation(self) -> None:
        self.ui.progressBar.setInvertedAppearance(False)
        self.animation_value = 0
        self.animation_reversed = False
        self.ui.progressBar.setInvertedAppearance(self.animation_reversed)
        if self.animation_timer:
            self.animation_timer.start(40)

    def stop_animation(self) -> None:
        self.ui.progressBar.setInvertedAppearance(False)
        if self.animation_timer:
            self.animation_timer.stop()

    def fetch_version(self) -> None:
        try:
            self.api.get_version()
        except InvalidVersion as e:
            QtWidgets.QMessageBox.critical(
                self,
                'Upgrade required',
                'A newer connector version is required.\nA browser will be opened to download it.',
                QtWidgets.QMessageBox.StandardButton.Ok,
            )
            webbrowser.open(e.downloadUrl)
            self.close_window()
            return
        except Exception as e:  # pylint: disable=broad-exception-caught
            if logger.getEffectiveLevel() == 10:
                logger.exception('Get Version')
            self.show_error(e)
            self.close_window()
            return

        self.fetch_transport_data()

    def fetch_transport_data(self) -> None:
        try:
            script, params = self.api.get_script_and_parameters(self.ticket, self.scrambler)
            self.stop_animation()

            if 'darwin' in sys.platform:
                self.showMinimized()

            # QtCore.QTimer.singleShot(3000, self.endScript)
            # self.hide()
            self.close_window()

            exec(script, globals(), {'parent': self, 'sp': params})  # pylint: disable=exec-used

            # Execute the waiting tasks...
            threading.Thread(target=end_script).start()

        except RetryException as e:
            self.ui.info.setText(str(e) + ', retrying access...')
            # Retry operation in ten seconds
            QtCore.QTimer.singleShot(10000, self.fetch_transport_data)
        except Exception as e:  # pylint: disable=broad-exception-caught
            if logger.getEffectiveLevel() == 10:
                logger.exception('Get Transport Data')
            self.show_error(e)

    def start(self) -> None:
        """
        Starts proccess by requesting version info
        """
        self.ui.info.setText('Initializing...')
        QtCore.QTimer.singleShot(100, self.fetch_version)


def end_script() -> None:
    # Wait a bit before start processing ending sequence
    time.sleep(3)
    try:
        # Remove early stage files...
        tools.unlink_files(early=True)
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Unlinking files on early stage: %s', e)

    # After running script, wait for stuff
    try:
        logger.debug('Wating for tasks to finish...')
        tools.waitForTasks()
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Watiting for tasks to finish: %s', e)

    try:
        logger.debug('Unlinking files')
        tools.unlink_files(early=False)
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Unlinking files on later stage: %s', e)

    # Removing
    try:
        logger.debug('Executing threads before exit')
        tools.exec_before_exit()
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('execBeforeExit: %s', e)

    logger.debug('endScript done')


# Ask user to approve endpoint
def verify_host_approval(hostName: str) -> bool:
    settings = QtCore.QSettings()
    settings.beginGroup('endpoints')

    # approved = settings.value(hostName, False).toBool()
    approved = bool(settings.value(hostName, False))

    errorString = '<p>The server <b>{}</b> must be approved:</p>'.format(hostName)
    errorString += '<p>Only approve UDS servers that you trust to avoid security issues.</p>'

    if not approved:
        if (
            QtWidgets.QMessageBox.warning(
                None,  # type: ignore
                'ACCESS Warning',
                errorString,
                QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No,
            )
            == QtWidgets.QMessageBox.StandardButton.Yes
        ):
            settings.setValue(hostName, True)
            approved = True

    settings.endGroup()
    return approved


def ssl_error_processor(hostname: str, serial: str) -> bool:
    settings = QSettings()
    settings.beginGroup('ssl')

    approved: bool = bool(settings.value(serial, False))

    if (
        approved
        or QtWidgets.QMessageBox.warning(
            None,  # type: ignore
            'SSL Warning',
            f'Could not check SSL certificate for {hostname}.\nDo you trust this host?',
            QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No,
        )
        == QtWidgets.QMessageBox.StandardButton.Yes
    ):
        approved = True
        settings.setValue(serial, True)

    settings.endGroup()
    return approved


# Used only if command line says so
def minimal(api: RestApi, ticket: str, scrambler: str) -> int:
    try:
        logger.info('Minimal Execution')
        logger.debug('Getting version')
        try:
            api.get_version()
        except InvalidVersion as e:
            QtWidgets.QMessageBox.critical(
                None,  # type: ignore
                'Upgrade required',
                'A newer connector version is required.\nA browser will be opened to download it.',
                QtWidgets.QMessageBox.StandardButton.Ok,
            )
            webbrowser.open(e.downloadUrl)
            return 0
        logger.debug('Transport data')
        script, params = api.get_script_and_parameters(ticket, scrambler)

        # Execute UDS transport script
        exec(script, globals(), {'parent': None, 'sp': params})
        # Execute the waiting task...
        threading.Thread(target=end_script).start()

    except RetryException as e:
        QtWidgets.QMessageBox.warning(
            None,  # type: ignore
            'Service not ready',
            '{}'.format('.\n'.join(str(e).split('.'))) + '\n\nPlease, retry again in a while.',
            QtWidgets.QMessageBox.StandardButton.Ok,
        )
    except Exception as e:  # pylint: disable=broad-exception-caught
        # logger.exception('Got exception on getTransportData')
        QtWidgets.QMessageBox.critical(
            None,  # type: ignore
            'Error',
            '{}'.format(str(e)) + '\n\nPlease, retry again in a while.',
            QtWidgets.QMessageBox.StandardButton.Ok,
        )
    return 0


def main(args: typing.List[str]) -> None:
    app = QtWidgets.QApplication(sys.argv)
    logger.debug('Initializing connector for %s(%s)', sys.platform, platform.machine())

    logger.debug('Arguments: %s', args)
    # Set several info for settings
    QtCore.QCoreApplication.setOrganizationName('Virtual Cable S.L.U.')
    QtCore.QCoreApplication.setApplicationName('UDS Connector')

    if 'darwin' not in sys.platform:
        logger.debug('Mac OS *NOT* Detected')
        app.setStyle('plastique')
    else:
        logger.debug('Platform is Mac OS, adding homebrew possible paths')
        os.environ['PATH'] += ''.join(
            os.pathsep + i
            for i in (
                '/usr/local/bin',
                '/opt/homebrew/bin',
            )
        )
        logger.debug('Now path is %s', os.environ['PATH'])

    # First parameter must be url
    _use_minimal_interface = False  # TBD: This is not used, but it is set to True in the minimal function
    try:
        uds_url = args[1]

        if uds_url == '--minimal':
            _use_minimal_interface = True
            uds_url = args[2]  # And get URI

        if uds_url == '--test':
            sys.exit(0)

        logger.debug('URI: %s', uds_url)
        # Shows error if using http (uds:// ) version, not supported anymore
        if uds_url[:6] == 'uds://' and not consts.DEBUG:
            QtWidgets.QMessageBox.critical(
                None,  # type: ignore
                'Notice',
                f'UDS Client Version {VERSION} does not support HTTP protocol Anymore.',
                QtWidgets.QMessageBox.StandardButton.Ok,
            )
            sys.exit(1)
        if uds_url[:7] != 'udss://':
            raise Exception('Not supported protocol')  # Just shows "about" dialog

        host, ticket, scrambler = uds_url.split('//')[1].split('/')
        logger.debug(
            'host:%s, ticket:%s, scrambler:%s',
            host,
            ticket,
            scrambler,
        )
    except Exception:  # pylint: disable=broad-except
        logger.debug('Detected execution without valid URI, exiting')
        QtWidgets.QMessageBox.critical(
            None,  # type: ignore
            'Notice',
            f'UDS Client Version {VERSION}',
            QtWidgets.QMessageBox.StandardButton.Ok,
        )
        sys.exit(1)

    # Setup REST api endpoint
    api = RestApi(f'https://{host}/uds/rest/client', ssl_error_processor)

    try:
        logger.debug('Starting execution')

        # Approbe before going on
        if verify_host_approval(host) is False:
            raise Exception('Host {} was not approved'.format(host))

        win = UDSClient(api, ticket, scrambler)
        win.show()

        win.start()

        exitVal = app.exec()
        logger.debug('Execution finished correctly')

    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.exception('Got an exception executing client:')
        exitVal = 128
        QtWidgets.QMessageBox.critical(
            None, 'Error', f'Fatal error: {e}', QtWidgets.QMessageBox.StandardButton.Ok  # type: ignore
        )

    logger.debug('Exiting')
    sys.exit(exitVal)


if __name__ == "__main__":
    main(sys.argv)