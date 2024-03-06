# -*- coding: utf-8 -*-
#
# Copyright (c) 2022 Virtual Cable S.L.U.
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
#    * Neither the name of Virtual Cable S.L. nor the names of its contributors
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
'''
Author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import logging
import typing
import sys
import os
from unittest import TestCase, mock

import UDSClient
from uds import exceptions, consts, rest

from .utils import fixtures

logger = logging.getLogger(__name__)


class TestClient(TestCase):
    def setUp(self) -> None:
        # If linux, and do not have X11, we will skip the tests
        if sys.platform == 'linux' and 'DISPLAY' not in os.environ:
            self.skipTest('Skipping test on linux without X11')

    def test_commandline(self) -> None:
        def _check_url(url: str, minimal: typing.Optional[str] = None, with_minimal: bool = False) -> None:
            host, ticket, scrambler, use_minimal = UDSClient.parse_arguments(
                ['udsclient'] + ([url] if not minimal else [minimal, url])
            )
            self.assertEqual(host, 'a')
            self.assertEqual(ticket, 'b')
            self.assertEqual(scrambler, 'c')
            self.assertEqual(use_minimal, with_minimal)

        # Invalid command line, should return simeple Exception
        with self.assertRaises(Exception):
            UDSClient.parse_arguments(['udsclient'])

        # Valid command line, but not an URI. should return UDSArgumentException
        with self.assertRaises(exceptions.ArgumentException):
            UDSClient.parse_arguments(['udsclient', '--test'])

        # unkonwn protocol, should return UDSArgumentException
        with self.assertRaises(exceptions.MessageException):
            UDSClient.parse_arguments(['udsclient', 'unknown://' + 'a' * 2048])

        # uds protocol, but withoout debug mode, should rais exception.UDSMessagException
        consts.DEBUG = False
        with self.assertRaises(exceptions.MessageException):
            _check_url('uds://a/b/c')

        # Set DEBUG mode (on consts), now should work
        consts.DEBUG = True
        _check_url('uds://a/b/c')

        # Now, a valid URI ssl (udss://)
        for debug in [True, False]:
            consts.DEBUG = debug
            _check_url('udss://a/b/c')
            _check_url('udss://a/b/c', '--minimal', with_minimal=True)
            # No matter what is passed as value of minimal, if present, it will be used
            _check_url('udss://a/b/c?minimal=11', with_minimal=True)

    def test_rest(self) -> None:
        # This is a simple test, we will test the rest api is mocked correctly
        with fixtures.patch_rest_api() as api:
            self.assertEqual(api.get_version(), fixtures.REQUIRED_VERSION)
            self.assertEqual(
                api.get_script_and_parameters('ticket', 'scrambler'), (fixtures.SCRIPT, fixtures.PARAMETERS)
            )

            from_api = rest.RestApi.api('host', lambda x, y: True)
            # Repeat tests, should return same results
            self.assertEqual(from_api.get_version(), fixtures.REQUIRED_VERSION)
            self.assertEqual(
                from_api.get_script_and_parameters('ticket', 'scrambler'),
                (fixtures.SCRIPT, fixtures.PARAMETERS),
            )
            # And also, the api is the same
            self.assertEqual(from_api, api)

    def test_udsclient(self) -> None:
        with fixtures.patched_uds_client() as client:
            # patch UDSClient module waiting_tasks_processor to avoid waiting for tasks
            with mock.patch('UDSClient.waiting_tasks_processor'):
                # Patch builting "exec"
                with mock.patch('builtins.exec') as builtins_exec:
                    # Desencadenate the full process
                    client.fetch_version()

                    # These are in fact mocks, but type checker does not know that
                    client.api.get_version.assert_called_with()  # type: ignore
                    client.api.get_script_and_parameters.assert_called_with(client.ticket, client.scrambler)  # type: ignore

                    # Builtin exec should be called with:
                    #  - The script
                    #  - The globals, because the globals in scripts may be different, we use mock.ANY
                    #  - The locals ->  {'parent': self, 'sp': params}, where self is the client and params is the parameters
                    builtins_exec.assert_called_with(
                        fixtures.SCRIPT, mock.ANY, {'parent': client, 'sp': fixtures.PARAMETERS}
                    )

                    # And also, process_waiting_tasks should be called, to process remaining tasks
                    client.process_waiting_tasks.assert_called_with()  # type: ignore

                    logger.debug('Testing fetch_script')

    def test_udsclient_invalid_version(self) -> None:
        with fixtures.patched_uds_client() as client:
            with mock.patch('webbrowser.open') as webbrowser_open:
                fixtures.REQUIRED_VERSION = '.'.join(
                    str(int(x) + 1) for x in consts.VERSION.split('.')
                )  # This will make the version greater than the required
                client.fetch_version()

                # error message should be called to show the required new version
                # but we do not check message content, just that it was called
                # It's an static method, so we can check it directly
                UDSClient.UDSClient.error_message.assert_called()  # type: ignore
                webbrowser_open.assert_called_with(fixtures.CLIENT_LINK)

    def test_udsclient_error_version(self) -> None:
        with fixtures.patched_uds_client() as client:
            with mock.patch('webbrowser.open') as webbrowser_open:
                fixtures.REQUIRED_VERSION = 'fail'
                client.fetch_version()

                # error message should be called to show problem checking version
                UDSClient.UDSClient.error_message.assert_called()  # type: ignore
                # webrowser should not be called
                webbrowser_open.assert_not_called()

                self.assertTrue(client.has_error)

    def test_fetch_transport_data(self) -> None:
        with fixtures.patched_uds_client() as client:
            client.fetch_transport_data()

            # error message should be called to show problem checking version
            UDSClient.UDSClient.error_message.assert_called()  # type: ignore

            self.assertTrue(client.has_error)

    def test_fetch_transport_data_retry(self) -> None:
        with fixtures.patched_uds_client() as client:
            with mock.patch('uds.ui.QtCore.QTimer.singleShot') as singleShot:
                fixtures.SCRIPT = 'retry'
                client.fetch_transport_data()

                # We should have a single shot timer to retry
                singleShot.assert_called_with(mock.ANY, client.fetch_transport_data)