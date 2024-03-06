# -*- coding: utf-8 -*-
#
# Copyright (c) 2017-2024 Virtual Cable S.L.U.
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
import contextlib
import typing
from unittest import mock

import UDSClient
from uds import consts, exceptions
from uds import rest, ui

from . import autospec

TESTING_VERSION: str = '4.0.0'
REQUIRED_VERSION: str = TESTING_VERSION
CLIENT_LINK: str = 'https://sample.client.link/udsclient.downloadable'
TESTING_SCRIPT: str = '''
# TODO: add testing script here
'''
SCRIPT: str = TESTING_SCRIPT
PARAMETERS: typing.MutableMapping[str, typing.Any] = {
    # TODO: add parameters here
}


def check_version() -> str:
    if REQUIRED_VERSION == 'fail':
        raise Exception('Version check failed miserably! :) (just for testing)')
    if consts.VERSION < REQUIRED_VERSION:
        raise exceptions.InvalidVersionException(CLIENT_LINK, REQUIRED_VERSION)
    return REQUIRED_VERSION


def script_and_parameters(
    ticket: str, scrambler: str
) -> typing.Tuple[str, typing.MutableMapping[str, typing.Any]]:
    global SCRIPT
    if SCRIPT == 'fail':
        raise Exception('Script retrieval failed miserably! :) (just for testing)')
    elif SCRIPT == 'retry':
        # Will not be on loop forever, because there will be only one call
        raise exceptions.RetryException('Just for testing')
    return SCRIPT, PARAMETERS


REST_METHODS_INFO: typing.List[autospec.AutoSpecMethodInfo] = [
    autospec.AutoSpecMethodInfo(rest.RestApi.get_version, method=check_version),
    autospec.AutoSpecMethodInfo(rest.RestApi.get_script_and_parameters, method=script_and_parameters),
]


def create_client_mock() -> mock.Mock:
    """
    Create a mock of ProxmoxClient
    """
    return autospec.autospec(rest.RestApi, REST_METHODS_INFO)


@contextlib.contextmanager
def patch_rest_api(
    **kwargs: typing.Any,
) -> typing.Generator['rest.RestApi', None, None]:
    client = create_client_mock()
    patcher = None
    try:
        patcher = mock.patch('uds.rest.RestApi.api', return_value=client)
        patcher.start()
        yield client
    finally:
        if patcher:
            patcher.stop()


@contextlib.contextmanager
def patched_uds_client() -> typing.Generator['UDSClient.UDSClient', None, None]:
    app = ui.QtWidgets.QApplication.instance() or ui.QtWidgets.QApplication([])
    with patch_rest_api() as client:
        uds_client = UDSClient.UDSClient(client, 'ticket', 'scrambler')
        # Now, patch object:
        # - process_waiting_tasks so we do not launch any task
        # - error_message so we do not show any error message
        # - warning_message so we do not show any warning message
        # error_message and warning_message are static methods, so we need to patch them on the class
        with mock.patch.object(uds_client, 'process_waiting_tasks'), mock.patch(
            'UDSClient.UDSClient.error_message'
        ), mock.patch('UDSClient.UDSClient.warning_message'):
            yield uds_client
    app.quit()
    del app
