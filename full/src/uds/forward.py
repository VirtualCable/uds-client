# Based on forward.py from paramiko
# Copyright (C) 2003-2007  Robey Pointer <robeypointer@gmail.com>
# https://github.com/paramiko/paramiko/blob/master/demos/forward.py


from binascii import hexlify
import threading
import random
import time
import select
import socketserver
import typing

import paramiko

from .log import logger


class CheckfingerPrints(paramiko.MissingHostKeyPolicy):
    def __init__(self, fingerPrints):
        super().__init__()
        self.fingerPrints = fingerPrints

    def missing_host_key(self, client, hostname, key):
        if self.fingerPrints:
            remotefingerPrints = hexlify(key.get_fingerprint()).decode().lower()
            if remotefingerPrints not in self.fingerPrints.split(','):
                logger.error(
                    "Server {!r} has invalid fingerPrints. ({} vs {})".format(
                        hostname, remotefingerPrints, self.fingerPrints
                    )
                )
                raise paramiko.SSHException(
                    "Server {!r} has invalid fingerPrints".format(hostname)
                )


class ForwardServer(socketserver.ThreadingTCPServer):
    daemon_threads = True
    allow_reuse_address = True


class Handler(socketserver.BaseRequestHandler):
    event: threading.Event
    thread: 'ForwardThread'
    ssh_transport: paramiko.Transport
    chain_host: str
    chain_port: int

    def handle(self):
        self.thread.currentConnections += 1

        try:
            chan = self.ssh_transport.open_channel(
                'direct-tcpip',
                (self.chain_host, self.chain_port),
                self.request.getpeername(),
            )
        except Exception as e:
            logger.exception(
                'Incoming request to %s:%d failed: %s',
                self.chain_host,
                self.chain_port,
                repr(e),
            )
            return
        if chan is None:
            logger.error(
                'Incoming request to %s:%d was rejected by the SSH server.',
                self.chain_host,
                self.chain_port,
            )
            return

        logger.debug(
            'Connected!  Tunnel open %r -> %r -> %r',
            self.request.getpeername(),
            chan.getpeername(),
            (self.chain_host, self.chain_port),
        )
        # self.ssh_transport.set_keepalive(10)  # Keep alive every 10 seconds...
        try:
            while self.event.is_set() is False:
                r, _w, _x = select.select(
                    [self.request, chan], [], [], 1
                )  # pylint: disable=unused-variable

                if self.request in r:
                    data = self.request.recv(1024)
                    if not data:
                        break
                    chan.send(data)
                if chan in r:
                    data = chan.recv(1024)
                    if not data:
                        break
                    self.request.send(data)
        except Exception:
            pass

        try:
            peername = self.request.getpeername()
            chan.close()
            self.request.close()
            logger.debug(
                'Tunnel closed from %r',
                peername,
            )
        except Exception:
            pass

        self.thread.currentConnections -= 1

        if self.thread.stoppable is True and self.thread.currentConnections == 0:
            self.thread.stop()


class ForwardThread(threading.Thread):
    status = 0  # Connecting
    client: typing.Optional[paramiko.SSHClient]
    fs: typing.Optional[ForwardServer]

    def __init__(
        self,
        server,
        port,
        username,
        password,
        localPort,
        redirectHost,
        redirectPort,
        waitTime,
        fingerPrints,
    ):
        threading.Thread.__init__(self)
        self.client = None
        self.fs = None

        self.server = server
        self.port = int(port)
        self.username = username
        self.password = password

        self.localPort = int(localPort)
        self.redirectHost = redirectHost
        self.redirectPort = redirectPort

        self.waitTime = waitTime

        self.fingerPrints = fingerPrints

        self.stopEvent = threading.Event()

        self.timer = None
        self.currentConnections = 0
        self.stoppable = False
        self.client = None

    def clone(self, redirectHost, redirectPort, localPort=None):
        if localPort is None:
            localPort = random.randrange(33000, 53000)

        ft = ForwardThread(
            self.server,
            self.port,
            self.username,
            self.password,
            localPort,
            redirectHost,
            redirectPort,
            self.waitTime,
            self.fingerPrints,
        )
        ft.client = self.client
        self.client.useCount += 1  # type: ignore
        ft.start()

        while ft.status == 0:
            time.sleep(0.1)

        return (ft, localPort)

    def _timerFnc(self):
        self.timer = None
        logger.debug('Timer fnc: %s', self.currentConnections)
        self.stoppable = True
        if self.currentConnections <= 0:
            self.stop()

    def run(self):
        if self.client is None:
            try:
                self.client = paramiko.SSHClient()
                self.client.useCount = 1  # type: ignore
                self.client.load_system_host_keys()
                self.client.set_missing_host_key_policy(
                    CheckfingerPrints(self.fingerPrints)
                )

                logger.debug('Connecting to ssh host %s:%d ...', self.server, self.port)

                # To disable ssh-ageng asking for passwords: allow_agent=False
                self.client.connect(
                    self.server,
                    self.port,
                    username=self.username,
                    password=self.password,
                    timeout=5,
                    allow_agent=False,
                )
            except Exception:
                logger.exception('Exception connecting: ')
                self.status = 2  # Error
                return

        class SubHandler(Handler):
            chain_host = self.redirectHost
            chain_port = self.redirectPort
            ssh_transport = self.client.get_transport()  # type: ignore
            event = self.stopEvent
            thread = self

        logger.debug('Wait Time: %s', self.waitTime)
        self.timer = threading.Timer(self.waitTime, self._timerFnc)
        self.timer.start()

        self.status = 1  # Ok, listening

        self.fs = ForwardServer(('', self.localPort), SubHandler)
        self.fs.serve_forever()

    def stop(self):
        try:
            if self.timer:
                self.timer.cancel()

            self.stopEvent.set()

            if self.fs:
                self.fs.shutdown()

            if self.client is not None:
                self.client.useCount -= 1  # type: ignore
                if self.client.useCount == 0:  # type: ignore
                    self.client.close()
                self.client = None  # Clean up
        except Exception:
            logger.exception('Exception stopping')


def forward(
    server,
    port,
    username,
    password,
    redirectHost,
    redirectPort,
    localPort=None,
    waitTime=10,
    fingerPrints=None,
):
    '''
    Instantiates an ssh connection to server:port
    Returns the Thread created and the local redirected port as a list: (thread, port)
    '''
    port, redirectPort = int(port), int(redirectPort)

    if localPort is None:
        localPort = random.randrange(40000, 50000)

    logger.debug(
        'Connecting to %s:%s using %s/%s redirecting to %s:%s, listening on 127.0.0.1:%s',
        server,
        port,
        username,
        password,
        redirectHost,
        redirectPort,
        localPort,
    )

    ft = ForwardThread(
        server,
        port,
        username,
        password,
        localPort,
        redirectHost,
        redirectPort,
        waitTime,
        fingerPrints,
    )

    ft.start()

    while ft.status == 0:
        time.sleep(0.1)

    return (ft, localPort)
