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
from unittest import TestCase

import UDSClient
from uds import exceptions, consts, rest

from .utils import fixtures

logger = logging.getLogger(__name__)


class TestClient(TestCase):
    def test_commandline(self):
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

    def test_rest(self):
        # This is a simple test, we will test the rest api is mocked correctly
        with fixtures.patch_rest_api() as api:
            self.assertEqual(api.get_version(), fixtures.SERVER_VERSION)
            self.assertEqual(api.get_script_and_parameters('ticket', 'scrambler'), (fixtures.SCRIPT, fixtures.PARAMETERS))
            
            from_api = rest.RestApi.api('host', lambda x, y: True)
            # Repeat tests, should return same results
            self.assertEqual(from_api.get_version(), fixtures.SERVER_VERSION)
            self.assertEqual(from_api.get_script_and_parameters('ticket', 'scrambler'), (fixtures.SCRIPT, fixtures.PARAMETERS))
            # And also, the api is the same
            self.assertEqual(from_api, api)
