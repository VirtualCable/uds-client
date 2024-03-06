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
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE

'''
Author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import socket
import threading
import typing

from uds import consts

from . import certs

MAX_EXEC_TIME: typing.Final[int] = 2  # Max execution time for the connections


class TunnelServer(threading.Thread):
    listening: threading.Event
    port: int
    error: bool
    error_msg: typing.Optional[str]
    ticket: typing.Optional[bytes]
    wait_time: int

    def __init__(self, wait_time: int = MAX_EXEC_TIME) -> None:
        super().__init__()
        self.wait_time = wait_time
        self.listening = threading.Event()
        self.port = 0
        self.error = False
        self.error_msg = None
        self.ticket = None

    def listen(self, server: socket.socket) -> socket.socket:
        server.settimeout(self.wait_time)  # So the task never gets stuck, this is for testing purposes only
        server.bind(('localhost', 0))
        self.port = server.getsockname()[1]
        server.listen(1)
        self.listening.set()
        conn, _addr = server.accept()
        conn.settimeout(self.wait_time)  # So the task never gets stuck, this is for testing purposes only
        return conn

    def read_header(self, conn: socket.socket) -> None:
        header = conn.recv(len(consts.HANDSHAKE_V1), socket.MSG_WAITALL)
        if header != consts.HANDSHAKE_V1:
            raise Exception(f'Invalid header: {header}')

    def process_command(self, conn: socket.socket) -> None:
        with certs.server_ssl_context() as ssl_context:
            # Upgrade connection to SSL
            conn = ssl_context.wrap_socket(conn, server_side=True)

            # Read command, 4 bytes (consts.CMD_OPEN or consts.CMD_TEST)
            command = conn.recv(4)  # conn is now ssl socket, does not allows non-zero flags
            if command == consts.CMD_OPEN:
                # Read the ticket
                self.ticket = conn.recv(consts.TICKET_LENGTH)
                conn.send(consts.RESPONSE_OK)
            elif command == consts.CMD_TEST:
                # Just return OK
                conn.send(consts.RESPONSE_OK)
            else:
                self.error = True
                self.error_msg = f'Invalid command: {command}'

    def run(self) -> None:
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
                conn = self.listen(server)
                self.read_header(conn)
                self.process_command(conn)
        except Exception as e:
            self.error = True
            self.error_msg = f'Exception: {e}'

    def wait_for_listener(self) -> None:
        self.listening.wait()
        if self.error:
            raise Exception(f'Error starting server: {self.error_msg}')
