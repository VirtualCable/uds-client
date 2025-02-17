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
import base64
import bz2
import json
import socket
import ssl
import typing
import urllib
import urllib.error
import urllib.parse
import urllib.request

from cryptography import x509
from cryptography.hazmat.backends import default_backend

from . import consts, tools, exceptions
from .log import logger

# Callback for error on cert
# parameters are hostname, serial
# If returns True, ignores error
CertCallbackType = typing.Callable[[str, str], bool]


class RestApi:

    _rest_api_endpoint: str  # base Rest API URL
    _on_invalid_certificate: typing.Optional[CertCallbackType]
    _required_version: str

    def __init__(
        self,
        rest_api_endpoint: str,
        on_invalid_certificate: typing.Optional[CertCallbackType] = None,
    ) -> None:  # parent not used
        logger.debug('Setting request URL to %s', rest_api_endpoint)

        self._rest_api_endpoint = rest_api_endpoint
        self._on_invalid_certificate = on_invalid_certificate
        self._required_version = ''

    def request(
        self,
        path: str,
        params: typing.Optional[typing.Mapping[str, str]] = None,
        data: typing.Optional[str] = None,
    ) -> typing.Any:
        if params:
            path += '?' + '&'.join(
                '{}={}'.format(k, urllib.parse.quote(str(v).encode('utf8'))) for k, v in params.items()
            )

        return json.loads(RestApi.request_url(self._rest_api_endpoint + path, self._on_invalid_certificate, data=data))

    def process_error(self, data: typing.Any) -> None:
        if 'error' in data:
            # Get retrayable from data, if not present, use old key
            if data.get('is_retrayable', data.get('retryable', '0')) == '1':
                raise exceptions.RetryException(data['error'])

            raise exceptions.UDSException(data['error'])

    def get_version(self) -> str:
        '''Gets and stores the serverVersion.
        Also checks that the version is valid for us. If not,
        will raise an "InvalidVersion' exception'''

        client_link = ''
        if not self._required_version:
            data = self.request('')  # Version is returned on 'main' path
            self.process_error(data)
            # get server version, using new key but, if not present, use old one
            # Note: old version will be removed on 5.0.0 (As all 4.0 brokers will already return the new keys)
            if 'requiredVersion' in data['result']:
                self._required_version = data['result']['requiredVersion']
                client_link = data['result']['downloadUrl']
            else:
                self._required_version = data['result']['required_version']
                client_link = data['result']['client_link']

        try:
            if self._required_version > consts.VERSION:
                raise exceptions.InvalidVersionException(client_link, self._required_version)

            return self._required_version
        except exceptions.InvalidVersionException:
            raise
        except Exception as e:
            raise exceptions.UDSException(e) from e

    def get_script_and_parameters(
        self, ticket: str, scrambler: str
    ) -> typing.Tuple[str, typing.Any, typing.Dict[str, typing.Any]]:
        '''Gets the transport script, validates it if necesary
        and returns it'''
        try:
            data = self.request(
                '/{}/{}'.format(ticket, scrambler),
                params={'hostname': tools.gethostname(), 'version': consts.VERSION},
            )
        except Exception as e:
            logger.exception('Got exception on getTransportData')
            raise e

        logger.debug('Transport data received')
        self.process_error(data)

        params = None

        res: typing.Dict[str, typing.Any] = data['result']
        # We have three elements on result:
        # * Script
        # * Signature
        # * Script data
        # We test that the Script has correct signature, and them execute it with the parameters
        # script, signature, params = res['script'].decode('base64').decode('bz2'), res['signature'], json.loads(res['params'].decode('base64').decode('bz2'))
        script, signature, params = (
            bz2.decompress(base64.b64decode(res['script'])),
            res['signature'],
            json.loads(bz2.decompress(base64.b64decode(res['params']))),
        )
        if tools.verify_signature(script, signature) is False:
            logger.error('Signature is invalid')

            raise Exception('Invalid UDS code signature. Please, report to administrator')

        return script.decode(), params, res.get('log', {})

        # exec(script.decode("utf-8"), globals(), {'parent': self, 'sp': params})

    def send_log(self, log_ticket: str, log_data: str) -> None:
        '''Sends log data to server'''
        try:
            data = self.request(
                '/{}/log'.format(log_ticket),
                data=json.dumps({'log': log_data}),
                params={'hostname': tools.gethostname(), 'version': consts.VERSION},
            )
        except Exception as e:
            logger.exception('Got exception on sendLog')
            raise e

        self.process_error(data)

    @staticmethod
    def _open(
        url: str, certErrorCallback: typing.Optional[CertCallbackType] = None, data: 'bytes|None' = None
    ) -> typing.Any:
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        # Disable SSLv2, SSLv3, TLSv1, TLSv1.1
        ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        ctx.set_ciphers(consts.SECURE_CIPHERS)

        # If we have the certificates file, we use it
        if tools.get_cacerts_file() is not None:
            ctx.load_verify_locations(tools.get_cacerts_file())
        hostname = urllib.parse.urlparse(url)[1]
        serial = ''

        port = ''
        if ':' in hostname:
            hostname, port = hostname.split(':')

        if url.startswith('https'):
            port = port or '443'
            with ctx.wrap_socket(
                socket.socket(socket.AF_INET, socket.SOCK_STREAM),
                server_hostname=hostname,
            ) as s:
                s.connect((hostname, int(port)))
                # Get binary certificate
                binCert = s.getpeercert(True)
                if binCert:
                    cert = x509.load_der_x509_certificate(binCert, default_backend())
                else:
                    raise Exception('Certificate not found!')

            serial = hex(cert.serial_number)[2:]

        response = None
        ctx.verify_mode = ssl.CERT_REQUIRED
        ctx.check_hostname = True

        def _open_url(url: str) -> typing.Any:
            # Generate the request with the headers
            req = urllib.request.Request(
                url,
                headers={
                    'User-Agent': consts.USER_AGENT,
                },
            )
            return urllib.request.urlopen(req, data, context=ctx)

        try:
            response = _open_url(url)
        except urllib.error.URLError as e:
            if isinstance(e.reason, ssl.SSLCertVerificationError):
                # Ask about invalid certificate
                if certErrorCallback:
                    if certErrorCallback(hostname, serial):
                        ctx.check_hostname = False
                        ctx.verify_mode = ssl.CERT_NONE
                        response = _open_url(url)
                else:
                    raise
            else:
                raise

        return response

    @staticmethod
    def api(host: str, on_invalid_certificate: CertCallbackType) -> 'RestApi':
        return RestApi(f'https://{host}/uds/rest/client', on_invalid_certificate)

    @staticmethod
    def request_url(
        url: str, cert_error_callback: typing.Optional[CertCallbackType] = None, data: 'str|None' = None
    ) -> bytes:
        with RestApi._open(url, cert_error_callback, data=data.encode() if data else None) as response:
            resp = response.read()

        return resp
