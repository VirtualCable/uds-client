# -*- coding: utf-8 -*-
#
# Copyright (c) 2014-2021 Virtual Cable S.L.U.
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
@author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import logging
import socket
import base64

import urllib.request

# This module allows to use proxyes to make socket connections
# It's simply initializes the connection and, after proxy (if required) is connected, returns the socket


def connect(host: str, port: int) -> socket.socket:
    # Get proxy settings
    proxy = urllib.request.getproxies().get('https', None)
    if proxy:
        # If proxy is set, connect to it and try to connect to host:port
        # First, extract proxy scheme to connect
        proxyScheme = proxy[:proxy.index('://')]
        # Remove scheme from proxy
        proxy = proxy[proxy.index('://') + 3:]
        
        # If user:password is present in proxy, it will be used
        if '@' in proxy:
            # Extract user:password
            proxyUserPass = proxy[:proxy.index('@')]
            # Remove user:password from proxy
            proxy = proxy[proxy.index('@') + 1:]
            # Encode user and password for basic auth on proxy
            proxyUserPass = base64.b64encode(proxyUserPass.encode('utf8')).decode('utf8')
        else:
            proxyUserPass = None

        # ProxyHost may be ipv4 or ipv6, so we need to split it
        proxyHost, proxyPort = proxy.rsplit(':', 1)
        proxyPort = int(proxyPort)
        if proxyHost.startswith('['):
            # ipv6
            proxyHost = proxyHost[1:-1]
        logging.debug('Connecting to proxy {}:{} to connect to {}:{}'.format(proxyHost, proxyPort, host, port))
        s = socket.socket(socket.AF_INET6 if ':' in proxyHost else socket.AF_INET, socket.SOCK_STREAM)
        s.connect((proxyHost, proxyPort))
        # if https proxy, we need to upgrade connection to https
        s.sendall(f'CONNECT {host}:{port} HTTP/1.1\r\n'.encode('utf8'))
        if proxyUserPass:
            s.sendall(f'Proxy-Authorization: Basic {proxyUserPass}\r\n'.encode('utf8'))
        s.sendall(b'\r\n')
        # Read response
        data = s.recv(4096)
        if not data.startswith(b'HTTP/1.1 200'):
            raise Exception('Proxy returned error: {}'.format(data))
        # Return socket
        return s
    else:
        # If no proxy is set, simply connect to host:port
        logging.debug('Connecting to {}:{}'.format(host, port))
        s = socket.socket(socket.AF_INET6 if ':' in host else socket.AF_INET, socket.SOCK_STREAM)
        s.connect((host, port))
        return s

if __name__ == "__main__":
    import os
    os.environ['http_proxy'] = 'http://proxy:3128'
    logging.basicConfig(level=logging.DEBUG)
    s = connect('www.google.com', 80)
    s.sendall(b'GET / HTTP/1.0\r\nHost: www.google.com\r\n\r\n')
    response = b''
    while True:
        data = s.recv(1024)
        if not data:
            break
        response += data
    s.close()
    print(response.decode('utf8'))