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
Author: Adolfo Gómez, dkmaster at dkmon dot com
'''
import typing
import enum
import sys


class OsType(enum.Enum):
    LINUX = 'Linux'
    WINDOWS = 'Windows'
    MACOS = 'MacOS'
    UNKNOWN = 'Unknown'

    DETECTED_SO = (
        LINUX
        if sys.platform.startswith('linux')
        else (
            WINDOWS
            if sys.platform.startswith('win')
            else MACOS if sys.platform.startswith('darwin') else UNKNOWN
        )
    )

    def __str__(self) -> str:
        return str(self.value)


# ForwarServer states
class ForwardState(enum.IntEnum):
    TUNNEL_LISTENING = 0
    TUNNEL_OPENING = 1
    TUNNEL_PROCESSING = 2
    TUNNEL_ERROR = 3


class RemovableFile(typing.NamedTuple):
    path: str
    early_stage: bool = False

class AwaitableTask(typing.NamedTuple):
    task: typing.Any
    wait_subprocesses: bool = False
    
