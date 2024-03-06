# -*- coding: utf-8 -*-
#
# Copyright (c) 2023 Virtual Cable S.L.U.
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
import typing
import os
import os.path
import tempfile
import logging
import sys

from . import types


# Feature enabler
def _feature_requested(env_var: str) -> bool:
    env_var_name = env_var.upper().replace('-', '_')
    if env_var_name not in os.environ:
        # Look for temp file with that name, if it exists, its true
        # or if a file in the home directory with that name exists
        if os.path.exists(os.path.join(tempfile.gettempdir(), env_var)) or os.path.exists(
            os.path.join(os.path.expanduser('~'), env_var)
        ):
            return True

    return os.getenv(env_var_name, 'false').lower() in ('true', 'yes', '1')


DEBUG: typing.Final[bool] = _feature_requested('uds-debug-on')
LOGLEVEL: typing.Final[int] = logging.DEBUG if DEBUG else logging.INFO

LOGFILE: typing.Final[str] = os.getenv(
    'UDS_LOG_FILE',
    (
        os.path.expanduser('~/udsclient.log')  # Linux or Mac on home folder
        if 'linux' in sys.platform or 'darwin' in sys.platform
        else os.path.join(tempfile.gettempdir(), 'udsclient.log')  # Windows or unknown on temp folder
    ),
)

# UDS Client version
VERSION: typing.Final[str] = '4.0.0'

# User agent
USER_AGENT: typing.Final[str] = f'UDSClient/{VERSION} ({types.OsType.DETECTED_SO})'

# Secure channel ciphers
SECURE_CIPHERS: typing.Final[str] = (
    'TLS_AES_256_GCM_SHA384'
    ':TLS_CHACHA20_POLY1305_SHA256'
    ':TLS_AES_128_GCM_SHA256'
    ':ECDHE-RSA-AES256-GCM-SHA384'
    ':ECDHE-RSA-AES128-GCM-SHA256'
    ':ECDHE-RSA-CHACHA20-POLY1305'
    ':ECDHE-ECDSA-AES128-GCM-SHA256'
    ':ECDHE-ECDSA-AES256-GCM-SHA384'
    ':ECDHE-ECDSA-CHACHA20-POLY1305'
)

# Public key for validating signed scripts
PUBLIC_KEY = b'''-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAuNURlGjBpqbglkTTg2lh
dU5qPbg9Q+RofoDDucGfrbY0pjB9ULgWXUetUWDZhFG241tNeKw+aYFTEorK5P+g
ud7h9KfyJ6huhzln9eyDu3k+kjKUIB1PLtA3lZLZnBx7nmrHRody1u5lRaLVplsb
FmcnptwYD+3jtJ2eK9ih935DYAkYS4vJFi2FO+npUQdYBZHPG/KwXLjP4oGOuZp0
pCTLiCXWGjqh2GWsTECby2upGS/ZNZ1r4Ymp4V2A6DZnN0C0xenHIY34FWYahbXF
ZGdr4DFBPdYde5Rb5aVKJQc/pWK0CV7LK6Krx0/PFc7OGg7ItdEuC7GSfPNV/ANt
5BEQNF5w2nUUsyN8ziOrNih+z6fWQujAAUZfpCCeV9ekbwXGhbRtdNkbAryE5vH6
eCE0iZ+cFsk72VScwLRiOhGNelMQ7mIMotNck3a0P15eaGJVE2JV0M/ag/Cnk0Lp
wI1uJQRAVqz9ZAwvF2SxM45vnrBn6TqqxbKnHCeiwstLDYG4fIhBwFxP3iMH9EqV
2+QXqdJW/wLenFjmXfxrjTRr+z9aYMIdtIkSpADIlbaJyTtuQpEdWnrlDS2b1IGd
Okbm65EebVzOxfje+8dRq9Uqwip8f/qmzFsIIsx3wPSvkKawFwb0G5h2HX5oJrk0
nVgtClKcDDlSaBsO875WDR0CAwEAAQ==
-----END PUBLIC KEY-----'''


# Variables for tunnel
BUFFER_SIZE: typing.Final[int] = 1024 * 16  # Max buffer length
LISTEN_ADDRESS: typing.Final[str] = '127.0.0.1'
LISTEN_ADDRESS_V6: typing.Final[str] = '::1'
RESPONSE_OK: typing.Final[bytes] = b'OK'

# Ticket length
TICKET_LENGTH: typing.Final[int] = 48

# Constants strings for protocol
HANDSHAKE_V1: typing.Final[bytes] = b'\x5AMGB\xA5\x01\x00'
CMD_TEST: typing.Final[bytes] = b'TEST'
CMD_OPEN: typing.Final[bytes] = b'OPEN'
