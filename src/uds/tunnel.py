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
@author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import contextlib
import socket
import socketserver
import ssl
import threading
import select
import time
import typing
import logging

from . import tools, consts, types


logger = logging.getLogger(__name__)


class ForwardServer(socketserver.ThreadingTCPServer):
    daemon_threads = True
    allow_reuse_address = True

    remote: typing.Tuple[str, int]
    remote_ipv6: bool
    ticket: str
    stop_flag: threading.Event
    can_stop: bool
    timer: typing.Optional[threading.Timer]
    check_certificate: bool
    keep_listening: bool
    current_connections: int
    status: types.ForwardState

    address_family = socket.AF_INET

    def __init__(
        self,
        remote: typing.Tuple[str, int],
        ticket: str,
        timeout: int = 0,
        local_port: int = 0,  # Use first available listen port if not specified
        check_certificate: bool = True,
        keep_listening: bool = False,
        ipv6_listen: bool = False,
        ipv6_remote: bool = False,
    ) -> None:
        # Negative values for timeout, means:
        #   * accept always connections, but if no connection is stablished on timeout
        #     (positive), stop the listener
        #
        # Note that this is for backwards compatibility, better use "keep_listening"
        if timeout < 0:
            keep_listening = True
            timeout = abs(timeout)

        if ipv6_listen:
            self.address_family = socket.AF_INET6

        # Binds and activate the server, so if local_port is 0, it will be assigned
        super().__init__(
            server_address=(consts.LISTEN_ADDRESS_V6 if ipv6_listen else consts.LISTEN_ADDRESS, local_port),
            RequestHandlerClass=Handler,
        )

        self.remote = remote
        self.remote_ipv6 = ipv6_remote or ':' in remote[0]  # if ':' in remote address, it's ipv6 (port is [1])
        self.ticket = ticket
        self.check_certificate = check_certificate
        self.keep_listening = keep_listening
        self.stop_flag = threading.Event()  # False initial
        self.current_connections = 0

        self.status = types.ForwardState.TUNNEL_LISTENING
        self.can_stop = False

        timeout = timeout or 60
        self.timer = threading.Timer(timeout, ForwardServer._set_stoppable, args=(self,))
        self.timer.start()

        logger.debug('Remote: %s', remote)
        logger.debug('Remote IPv6: %s', self.remote_ipv6)
        logger.debug('Ticket: %s', ticket)
        logger.debug('Check certificate: %s', check_certificate)
        logger.debug('Keep listening: %s', keep_listening)
        logger.debug('Timeout: %s', timeout)

    def stop(self) -> None:
        if not self.stop_flag.is_set():
            logger.debug('Stopping servers')
            self.stop_flag.set()
            if self.timer:
                self.timer.cancel()
                self.timer = None
            self.shutdown()

    @contextlib.contextmanager
    def connection(self) -> typing.Generator[ssl.SSLSocket, None, None]:
        ssl_sock: typing.Optional[ssl.SSLSocket] = None
        try:
            ssl_sock = ForwardServer._connect(self.remote, self.remote_ipv6, self.check_certificate)
            yield ssl_sock
        finally:
            if ssl_sock:
                ssl_sock.close()

    def check(self) -> bool:
        if self.status == types.ForwardState.TUNNEL_ERROR:
            return False

        logger.debug('Checking tunnel availability')

        with self.connection() as ssl_socket:
            return ForwardServer._test(ssl_socket)

    @contextlib.contextmanager
    def open_tunnel(self) -> typing.Generator[ssl.SSLSocket, None, None]:
        self.current_connections += 1
        # Open remote connection
        try:
            with self.connection() as ssl_socket:
                ForwardServer._open_tunnel(ssl_socket, self.ticket)

                yield ssl_socket
        except ssl.SSLError as e:
            logger.error(f'Certificate error connecting to {self.remote!s}: {e!s}')
            self.status = types.ForwardState.TUNNEL_ERROR
            self.stop()
        except Exception as e:
            logger.error(f'Error connecting to {self.remote!s}: {e!s}')
            self.status = types.ForwardState.TUNNEL_ERROR
            self.stop()
        finally:
            self.current_connections -= 1

    @property
    def stoppable(self) -> bool:
        logger.debug('Is stoppable: %s', self.can_stop)
        return self.can_stop

    @staticmethod
    def _set_stoppable(fs: 'ForwardServer') -> None:
        # As soon as the timer is fired, the server can be stopped
        # This means that:
        #  * If not connections are stablished, the server will be stopped
        #  * If no "keep_listening" is set, the server will not allow any new connections
        logger.debug('New connection limit reached')
        fs.timer = None
        fs.can_stop = True
        # If timer fired, and no connections are stablished, stop the server
        if fs.current_connections <= 0:
            fs.stop()

    @staticmethod
    def _test(ssl_socket: ssl.SSLSocket) -> bool:
        try:
            ssl_socket.sendall(consts.CMD_TEST)
            resp = ssl_socket.recv(2)
            if resp != consts.RESPONSE_OK:
                raise Exception({'Invalid  tunnelresponse: {resp}'})
            logger.debug('Tunnel is available!')
            return True
        except ssl.SSLError as e:
            logger.error(f'Certificate error connecting to {ssl_socket.getsockname()}: {e!s}')
            # will surpas the "check" method on script caller, arriving to the UDSClient error handler
            raise Exception(f'Certificate error connecting to {ssl_socket.getsockname()}') from e
        except Exception as e:
            logger.error('Error connecting to tunnel server %s: %s', ssl_socket.getsockname(), e)
        return False

    @staticmethod
    def _open_tunnel(ssl_socket: ssl.SSLSocket, ticket: str) -> None:
        # Send handhshake + command + ticket
        ssl_socket.sendall(consts.CMD_OPEN + ticket.encode())
        # Check response is OK
        data = ssl_socket.recv(2)
        if data != consts.RESPONSE_OK:
            data += ssl_socket.recv(128)
            raise Exception(f'Error received: {data.decode(errors="ignore")}')  # Notify error

    @staticmethod
    def _connect(
        remote_addr: typing.Tuple[str, int],
        use_ipv6: bool = False,
        check_certificate: bool = True,
    ) -> ssl.SSLSocket:
        with socket.socket(socket.AF_INET6 if use_ipv6 else socket.AF_INET, socket.SOCK_STREAM) as rsocket:
            logger.info('CONNECT to %s', remote_addr)

            rsocket.connect(remote_addr)

            rsocket.sendall(consts.HANDSHAKE_V1)  # No response expected, just the handshake

            # Now, upgrade to ssl
            context = ssl.create_default_context()

            # Do not "recompress" data, use only "base protocol" compression
            context.options |= ssl.OP_NO_COMPRESSION
            # Macs with default installed python, does not support mininum tls version set to TLSv1.3
            # USe "brew" version instead, or uncomment next line and comment the next one
            # context.minimum_version = ssl.TLSVersion.TLSv1_2 if tools.isMac() else ssl.TLSVersion.TLSv1_3
            # Disallow old versions of TLS
            # context.minimum_version = ssl.TLSVersion.TLSv1_2
            # Secure ciphers, use this is enabled tls 1.2
            # context.set_ciphers('ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-SHA384')

            context.minimum_version = ssl.TLSVersion.TLSv1_3

            if tools.get_cacerts_file() is not None:
                context.load_verify_locations(tools.get_cacerts_file())  # Load certifi certificates

            # If ignore remote certificate
            if check_certificate is False:
                context.check_hostname = False
                context.verify_mode = ssl.CERT_NONE
                logger.warning('Certificate checking is disabled!')

            return context.wrap_socket(rsocket, server_hostname=remote_addr[0])


class Handler(socketserver.BaseRequestHandler):
    # Override Base type
    server: ForwardServer  # pyright: ignore[reportIncompatibleVariableOverride]

    def handle(self) -> None:
        if self.server.status == types.ForwardState.TUNNEL_LISTENING:
            self.server.status = types.ForwardState.TUNNEL_OPENING  # Only update state on first connection

        # If server new connections processing are over time...
        if self.server.stoppable and not self.server.keep_listening:
            self.server.status = types.ForwardState.TUNNEL_ERROR
            logger.error('Rejected timedout connection')
            self.request.close()  # End connection without processing it
            return

        # Open remote connection
        self.establish_and_handle_tunnel()

        # If no more connections are stablished, and server is stoppable, do it now
        if self.server.current_connections <= 0 and self.server.stoppable:
            self.server.stop()

    def establish_and_handle_tunnel(self) -> None:
        # Open remote connection
        try:
            # If the tunnel open fails, will raise an exception
            # and the tunnel will be closed
            # if the tunnel is opened, but some error handling connection happens,
            # the tunnel will be try to be re-opened (where it can give an exception, and the tunnel will be closed)
            while True:
                with self.server.open_tunnel() as ssl_socket:
                    try:
                        self.handle_tunnel(remote=ssl_socket)
                        break
                    except Exception as e:
                        logger.error('Remote connection failure: %s. Retrying...', e)
                        time.sleep(1)   # Wait a bit before retrying
        # All these exceptions are from the tunnel opening process
        except ssl.SSLError as e:
            logger.error(f'Certificate error connecting to {self.server.remote!s}: {e!s}')
            self.server.status = types.ForwardState.TUNNEL_ERROR
            self.server.stop()
        except Exception as e:
            logger.error(f'Error connecting to {self.server.remote!s}: {e!s}')
            self.server.status = types.ForwardState.TUNNEL_ERROR
            self.server.stop()
        finally:
            self.server.current_connections -= 1

    # Processes data forwarding
    def handle_tunnel(self, remote: ssl.SSLSocket) -> None:
        self.server.status = types.ForwardState.TUNNEL_PROCESSING
        logger.debug('Processing tunnel with ticket %s', self.server.ticket)
        # Process data until stop requested or connection closed
        try:
            while not self.server.stop_flag.is_set():
                # Wait for data from either side
                r, _w, _x = select.select([self.request, remote], [], [], 1.0)
                if self.request in r:  # If request (local) has data, send to remote
                    data = self.request.recv(consts.BUFFER_SIZE)
                    if not data:
                        break
                    remote.sendall(data)
                if remote in r:  # If remote has data, send to request (local)
                    data = remote.recv(consts.BUFFER_SIZE)
                    if not data:
                        break
                    self.request.sendall(data)
            logger.debug('Finished tunnel with ticket %s', self.server.ticket)
        except Exception:
            raise


def _run(server: ForwardServer) -> None:
    """
    Runs the forwarder server.
    This method is intended to be run in a separate thread.

    Args:
        server (ForwardServer): The forward server instance.

    Returns:
        None
    """

    def _runner() -> None:
        logger.debug(
            'Starting forwarder: %s -> %s',
            server.server_address,
            server.remote,
        )
        server.serve_forever()
        logger.debug('Stopped forwarder %s -> %s', server.server_address, server.remote)

    threading.Thread(target=_runner).start()


def forward(
    remote: typing.Tuple[str, int],
    ticket: str,
    timeout: int = 0,
    local_port: int = 0,
    check_certificate: bool = True,
    keep_listening: bool = True,
    use_ipv6: bool = False,
) -> ForwardServer:
    """
    Forward a connection to a remote server.

    Args:
        remote (Tuple[str, int]): The address and port of the remote server.
        ticket (str): The ticket used for authentication.
        timeout (int, optional): When the server will stop listening for new connections (default is 0, which means never).
        local_port (int, optional): The local port to bind to (default is 0, which means any available port).
        check_certificate (bool, optional): Whether to check the server's SSL certificate (default is True).
        keep_listening (bool, optional): Whether to keep listening for new connections (default is True).

    Returns:
        ForwardServer: An instance of the ForwardServer class.

    """
    fs = ForwardServer(
        remote=remote,
        ticket=ticket,
        timeout=timeout,
        local_port=local_port,
        check_certificate=check_certificate,
        ipv6_remote=use_ipv6,
        keep_listening=keep_listening,
    )
    # Starts a new thread for processing the server,
    # so the main thread can continue processing other tasks
    _run(fs)

    return fs
