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
import contextlib
import logging
import sys
import os
import platform
import time
import webbrowser
import threading
import urllib.parse
import typing

from uds.ui import QtCore, QtWidgets, QtGui, QSettings, Ui_MainWindow  # pyright: ignore
from uds.rest import RestApi

# Just to ensure there are available on runtime
from uds.tunnel import forward as tunnel_forwards  # pyright: ignore[reportUnusedImport]

from uds.log import logger, get_remote_log, init_remote_log
from uds import consts, exceptions, tools
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
        self.animation_timer.timeout.connect(self.update_anim)
        # QtCore.QObject.connect(self.animTimer, QtCore.SIGNAL('timeout()'), self.updateAnim)

        self.start_animation()

    def close_window(self) -> None:
        self.close()

    def show_error(self, error: Exception) -> None:
        logger.error('Error: %s', error)
        self.stop_animation()
        # In fact, main window is hidden, so this is not visible... :)
        self.ui.info.setText('UDS Plugin Error')
        self.close_window()
        UDSClient.error_message('UDS Plugin Error', f'{error}')
        self.has_error = True

    def on_cancel_pressed(self) -> None:
        self.close()

    def update_anim(self) -> None:
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
        except exceptions.InvalidVersionException as e:
            UDSClient.error_message(
                'Upgrade required',
                f'UDS Client version {e.required_version} is required.\nA browser will be opened to download it.',
            )
            webbrowser.open(e.link)
            self.close_window()
            return
        except Exception as e:
            if logger.getEffectiveLevel() == logging.DEBUG:
                logger.exception('Get Version')
            self.show_error(e)
            self.close_window()
            return

        self.fetch_transport_data()

    def fetch_transport_data(self) -> None:
        try:
            script, params, log_data = self.api.get_script_and_parameters(self.ticket, self.scrambler)

            init_remote_log(log_data)  # Initialize for remote logging if requested by server

            self.stop_animation()

            if 'darwin' in sys.platform:
                self.showMinimized()

            self.close_window()

            # Execute UDS transport script, signed and checked
            vars = {
                '__builtins__': __builtins__,
                'parent': self,
                'sp': params,
            }
            exec(script, vars)

            self.process_waiting_tasks()

        except exceptions.RetryException as e:
            self.ui.info.setText(str(e) + ', retrying access...')
            # Retry operation in ten seconds
            QtCore.QTimer.singleShot(10000, self.fetch_transport_data)
        except Exception as e:
            # If debug is enabled, show exception
            if logger.getEffectiveLevel() == logging.DEBUG:
                logger.exception('Get Transport Data')
            self.show_error(e)

    def process_waiting_tasks(self) -> None:
        """
        Process the waiting tasks in a separate thread.

        This way, the gui don't get blocked by the waiting tasks.

        Returns:
            None
        """

        threading.Thread(target=waiting_tasks_processor, args=(self.api,)).start()
        # And simply return

    def start(self) -> None:
        """
        Starts proccess by requesting version info
        """
        self.ui.info.setText('Initializing...')
        QtCore.QTimer.singleShot(100, self.fetch_version)  # Will make it async, not blocking the gui

    @staticmethod
    def warning_message(title: str, message: str, *, yes_no: bool = False) -> bool:
        buttons = (
            QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No
            if yes_no
            else QtWidgets.QMessageBox.StandardButton.Ok
        )
        return (
            QtWidgets.QMessageBox.warning(
                typing.cast(QtWidgets.QWidget, None),
                title,
                message,
                buttons,
            )
            == QtWidgets.QMessageBox.StandardButton.Yes  # If no yes_no, does not matter this comparison
        )

    @staticmethod
    def error_message(title: str, message: str, *, yes_no: bool = False) -> bool:
        buttons = (
            QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No
            if yes_no
            else QtWidgets.QMessageBox.StandardButton.Ok
        )
        return (
            QtWidgets.QMessageBox.critical(
                typing.cast(QtWidgets.QWidget, None),
                title,
                message,
                buttons,
            )
            == QtWidgets.QMessageBox.StandardButton.Yes  # If no yes_no, does not matter this comparison
        )

    @staticmethod
    @contextlib.contextmanager
    def settings(group: str) -> typing.Iterator[QSettings]:
        settings = QSettings()
        settings.beginGroup(group)
        yield settings
        settings.endGroup()


def waiting_tasks_processor(api: RestApi) -> None:
    # Wait a bit before start processing ending sequence
    time.sleep(3)
    try:
        # Remove early stage files...
        tools.unlink_files(early_stage=True)
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Unlinking files on early stage: %s', e)

    # After running script, wait for stuff
    try:
        logger.debug('Wating for tasks to finish...')
        tools.wait_for_tasks()
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Watiting for tasks to finish: %s', e)

    try:
        logger.debug('Unlinking files')
        tools.unlink_files(early_stage=False)
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('Unlinking files on later stage: %s', e)

    # Removing
    try:
        logger.debug('Executing threads before exit')
        tools.execute_before_exit()
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.debug('execBeforeExit: %s', e)

    logger.debug('endScript done')
    
    # Process remote logging if requested
    try:
        log_ticket, log_data = get_remote_log()
        logger.debug('** Remote log data: %s, %s', log_ticket, len(log_data))
        if log_ticket != '' and len(log_data) > 0:
            logger.debug('** Sending log data: %s, %s', log_ticket, len(log_data))
            api.send_log(log_ticket, log_data[-65536:])  # Limit to 64K
    except Exception as e:  # pylint: disable=broad-exception-caught
        logger.error('** Error sending log data: %s', e)


# Ask user to approve endpoint
def verify_host_approval(hostname: str) -> bool:
    with UDSClient.settings('endpoints') as settings:
        approved = bool(settings.value(hostname, False))

        errorString = '<p>The server <b>{}</b> must be approved:</p>'.format(hostname)
        errorString += '<p>Only approve UDS servers that you trust to avoid security issues.</p>'

        if not approved:
            if UDSClient.warning_message('ACCESS Warning', errorString, yes_no=True):
                settings.setValue(hostname, True)
                approved = True

    return approved


def ssl_certificate_validator(hostname: str, serial: str) -> bool:
    with UDSClient.settings('ssl') as settings:
        approved: bool = bool(settings.value(serial, False))
        if approved or UDSClient.warning_message(
            'SSL Warning',
            f'Could not check SSL certificate for {hostname}.\nDo you trust this host?',
            yes_no=True,
        ):
            approved = True
            settings.setValue(serial, True)

    return approved


# Used only if command line says so
def minimal(api: RestApi, ticket: str, scrambler: str) -> int:
    try:
        logger.info('Minimal Execution')
        logger.debug('Getting version')
        try:
            api.get_version()
        except exceptions.InvalidVersionException as e:
            UDSClient.error_message(
                'Upgrade required',
                'A newer connector version is required.\nA browser will be opened to download it.',
            )
            webbrowser.open(e.link)
            return 0
        logger.debug('Transport data')

        script, params, log_data = api.get_script_and_parameters(ticket, scrambler)

        init_remote_log(log_data)  # Initialize for remote logging if requested by server

        # A catch-all-calls class, to avoid errors on script becasue no parent
        class CatchAll:
            def __getattr__(self, name: str) -> 'CatchAll':
                return self

            def __call__(self, *args: typing.Any, **kwargs: typing.Any) -> 'CatchAll':
                return self

        vars = {
            '__builtins__': __builtins__,
            'parent': CatchAll(),
            'sp': params,
        }
        exec(script, vars)

        threading.Thread(target=waiting_tasks_processor, args=(api,)).start()

    except exceptions.RetryException as e:
        UDSClient.error_message(
            'Service not ready',
            '{}'.format('.\n'.join(str(e).split('.'))) + '\n\nPlease, retry again in a while.',
        )
    except Exception as e:  # pylint: disable=broad-exception-caught
        # logger.exception('Got exception on getTransportData')
        UDSClient.error_message('Error', f'{e}\n\nPlease, retry again in a while.')
    return 0


def parse_arguments(args: typing.List[str]) -> typing.Tuple[str, str, str, bool]:
    """
    Parses the command line arguments and returns a tuple containing the host, ticket, scrambler, and a flag indicating whether to use the minimal interface.

    Args:
        args (List[str]): The command line arguments. (including the program name as the first argument)

    Returns:
        Tuple[str, str, str, bool]: A tuple containing the host, ticket, scrambler, and a flag indicating whether to use the minimal interface.

    Raises:
        Exception: If the number of arguments is less than 2.
        IDSArgumentException: If the uds_url is '--test'.
        UDSMessageException: If the uds_url starts with 'uds://' and the DEBUG flag is False.
        UDSMessageException: If the uds_url does not start with 'udss://'.
    """
    if len(args) < 2:
        raise Exception()

    use_minimal_interface = False
    uds_url = args[1]

    if uds_url == '--minimal':
        use_minimal_interface = True
        uds_url = args[2]  # And get URI

    if uds_url == '--test':
        raise exceptions.ArgumentException('test')

    try:
        urlinfo = urllib.parse.urlparse(uds_url)
        ticket, scrambler = urlinfo.path.split('/')[1:3]
    except Exception:
        raise exceptions.MessageException('Invalid UDS URL')

    # Check if minimal interface is requested on the URL
    if 'minimal' in urllib.parse.parse_qs(urlinfo.query):
        use_minimal_interface = True

    if urlinfo.scheme == 'uds':
        if not consts.DEBUG:
            raise exceptions.MessageException(
                'UDS Client Version {} does not support HTTP protocol Anymore.'.format(VERSION)
            )
    elif urlinfo.scheme != 'udss':
        raise exceptions.MessageException('Not supported protocol')  # Just shows "about" dialog

    # If ticket length is not valid
    if len(ticket) != consts.TICKET_LENGTH:
        raise exceptions.MessageException(f'Invalid ticket: {ticket}')

    return (
        urlinfo.netloc,
        ticket,
        scrambler,
        use_minimal_interface,
    )


def main(args: typing.List[str]) -> int:
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
        logger.debug('Platform is Mac OS, adding homebrew well known paths')
        os.environ['PATH'] += ''.join(
            os.pathsep + i
            for i in (
                '/opt/homebrew/bin',
                '/usr/local/bin',
            )
        )
        logger.debug('Now path is %s', os.environ['PATH'])

    # First parameter must be url
    try:
        host, ticket, scrambler, _use_minimal_interface = parse_arguments(args)
    except exceptions.MessageException as e:
        logger.debug('Detected execution without valid URI, exiting: %s', e)
        UDSClient.error_message(
            f'UDS Client Version {VERSION}',
            f'{e}',
        )
        return 1
    except exceptions.ArgumentException as e:
        # Currently only test, return 0
        return 0
    except Exception:
        logger.debug('Detected execution without valid URI, exiting')
        UDSClient.error_message(
            'Notice',
            f'UDS Client Version {VERSION}',
        )

        return 1

    # Setup REST api and ssl certificate validator
    api = RestApi.api(
        host,
        on_invalid_certificate=ssl_certificate_validator,
    )

    try:
        logger.debug('Starting execution')

        # Approbe before going on
        if verify_host_approval(host) is False:
            raise exceptions.MessageException('Host {} was not approved'.format(host))

        win = UDSClient(api, ticket, scrambler)
        # Show and activate window, so it's on top
        win.show()
        win.activateWindow()

        win.start()

        exit_code = app.exec()
        logger.debug('Main execution finished correctly: %s', exit_code)

    except Exception as e:
        if not isinstance(e, exceptions.MessageException):
            logger.exception('Got an exception executing client:')
        else:
            logger.info('Message from error: %s', e)
        exit_code = 128
        UDSClient.error_message(
            'Error',
            f'Fatal error: {e}',
        )

    logger.debug('Exiting')
    return exit_code


if __name__ == "__main__":
    exit_code = main(sys.argv)
    # Set exit code
    sys.exit(exit_code)
