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
import os
import tempfile
import typing

import ssl

# Self-signed certificate and key for testing purposes
# openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 36500 -nodes -subj "/C=ES/ST=Madrid/L=Madrid/O=UDS/OU=Devel/CN=localhost"
CERT: typing.Final[str] = (
    '-----BEGIN CERTIFICATE-----\n'
    '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\n'
    '6GgWRVlAbVZ3\n'
    '-----END CERTIFICATE-----'
)

KEY: typing.Final[str] = (
    '-----BEGIN PRIVATE KEY-----\n'
    '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\n'
    'VH6zbSCVcHmtc2P8xA9VxERkeeauY8w=\n'
    '-----END PRIVATE KEY-----'
)


@contextlib.contextmanager
def server_ssl_context() -> typing.Generator[ssl.SSLContext, None, None]:
    certfile = tempfile.mktemp() + '.pem'
    keyfile = tempfile.mktemp() + '.pem'
    try:
        with open(certfile, 'w') as f:
            f.write(CERT)
        with open(keyfile, 'w') as f:
            f.write(KEY)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(certfile=certfile, keyfile=keyfile)
        yield ctx
    finally:
        # Remove the temporary files if they were created
        for fname in (certfile, keyfile):
            if os.path.exists(fname):
                os.unlink(fname)
