# -*- coding: utf-8 -*-
#
# Copyright (c) 2024 Virtual Cable S.L.U.
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
import logging
import socket
import os
import ssl
import tempfile
import time
import typing
from unittest import TestCase, mock

from uds import tunnel

from .utils import tunnel_server, certs, fixtures

logger = logging.getLogger(__name__)


class TestTunnel(TestCase):
    _server: typing.Optional['tunnel_server.TunnelServer'] = None

    def setUp(self) -> None:
        super().setUp()

    def tearDown(self) -> None:
        super().tearDown()
        if self._server and self._server.is_alive():
            self._server.join()

    @property
    def server(self) -> 'tunnel_server.TunnelServer':
        if self._server is None:
            self._server = tunnel_server.TunnelServer()
            self._server.start()
            self._server.listening.wait()
        return self._server

    @contextlib.contextmanager
    def connect(
        self, check_certificate: bool = False, use_ipv6: bool = False
    ) -> typing.Iterator[ssl.SSLSocket]:
        yield tunnel.ForwardServer._connect(
            ('localhost', self.server.port), use_ipv6=use_ipv6, check_certificate=check_certificate
        )

    @contextlib.contextmanager
    def ensure_valid_cert(self) -> typing.Iterator[None]:
        """Ensure that the certificate is valid by using a temporary file with the self-signed certificate.
        (Note: all self signed certificates are also valid CA certificates, so we can use it as a CA certificate file)
        """
        certfile = tempfile.NamedTemporaryFile('w', delete=False)
        certfile.write(certs.CERT)
        certfile.close()
        # mock tools.get_cacerts_file to point to certfile
        try:
            with mock.patch('uds.tools.get_cacerts_file', return_value=certfile.name):
                yield
        finally:
            if os.path.exists(certfile.name):
                os.unlink(certfile.name)

    def test_test_verify_cert_fails(self) -> None:
        # Should raise an exception if check_certificate is True, because certificate is self-signed
        with self.assertRaises(ssl.CertificateError):
            with self.connect(check_certificate=True):
                pass  # Just to make the test run

    def test_test_verify_cert(self) -> None:
        # mock toolsget_cacerts_file to point to
        with self.ensure_valid_cert():
            with self.connect(check_certificate=True) as conn:
                self.assertTrue(tunnel.ForwardServer._test(conn))

        self.assertFalse(self.server.error, self.server.error_msg)

    def test_test_no_verify_cert(self) -> None:
        with self.connect(check_certificate=False) as conn:
            self.assertTrue(tunnel.ForwardServer._test(conn))

        self.assertFalse(self.server.error, self.server.error_msg)

    def test_open_tunnel(self) -> None:
        with self.ensure_valid_cert():
            with self.connect() as conn:
                tunnel.ForwardServer._open_tunnel(conn, fixtures.TICKET)

        self.assertFalse(self.server.error, self.server.error_msg)

    def test_forward_fnc(self) -> None:
        """Check that forward function works as expected
        * Creates a thread that invokes tunnel._run
        """
        with mock.patch('uds.tunnel._run') as run:
            with mock.patch('uds.tunnel.ForwardServer') as ForwardServer:
                fs = tunnel.forward(
                    ('localhost', 1234), fixtures.TICKET, 1, 1222, check_certificate=False, use_ipv6=False
                )
                # Ensure that thread is invoked with _run as target, and fs as argument
                run.assert_called_once_with(fs)
                # And that ForwardServer is called with the correct parameters
                ForwardServer.assert_called_once_with(
                    remote=('localhost', 1234),
                    ticket=fixtures.TICKET,
                    timeout=1,
                    local_port=1222,
                    check_certificate=False,
                    ipv6_remote=False,
                    keep_listening=True,
                )

    def test_forward_stoppable(self) -> None:
        # Patch fs._set_stoppable to check if it is called
        with mock.patch('uds.tunnel.ForwardServer._set_stoppable') as _set_stoppable:
            fs = tunnel.forward(
                ('localhost', self.server.port), fixtures.TICKET, 1, 1222, check_certificate=False
            )

            time.sleep(1.1)  # more than forward timeout (1)
            self.assertTrue(_set_stoppable.called)

        fs.stop()  # Ensure fs is stopped

    def test_forward_connect(self) -> None:
        # Must be listening on 1222, so we can connect to it to make the tunnel start
        fs = tunnel.forward(('localhost', self.server.port), fixtures.TICKET, 1, 1222, check_certificate=False)
        with contextlib.closing(socket.socket()) as s:
            s.connect(('localhost', 1222))
            # Do net send anything, will not be read, just to make the tunnel start
            s.send(b'')

        self.assertFalse(self.server.error, self.server.error_msg)
        # Ensure fs is stopped
        fs.stop()
