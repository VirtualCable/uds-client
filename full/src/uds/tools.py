# -*- coding: utf-8 -*-
#
# Copyright (c) 2015-2021 Virtual Cable S.L.U.
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
import base64
import os
import os.path
import random
import socket
import stat
import string
import sys
import tempfile
import time
import typing

import certifi

# For signature checking
from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding

import psutil

from . import consts
from .log import logger

_unlinkFiles: typing.List[typing.Tuple[str, bool]] = []
_tasks_to_Wait: typing.List[typing.Tuple[typing.Any, bool]] = []
_execBeforeExit: typing.List[typing.Callable[[], None]] = []

sys_fs_enc = sys.getfilesystemencoding() or 'mbcs'


def save_temp_file(content: str, filename: typing.Optional[str] = None) -> str:
    if filename is None:
        filename = ''.join(random.choice(string.ascii_lowercase + string.digits) for _ in range(16))
        filename = filename + '.uds'

    filename = os.path.join(tempfile.gettempdir(), filename)

    with open(filename, 'w') as f:
        f.write(content)

    logger.info('Returning filename')
    return filename


def read_temp_file(filename: str) -> typing.Optional[str]:
    filename = os.path.join(tempfile.gettempdir(), filename)
    try:
        with open(filename, 'r') as f:
            return f.read()
    except Exception:
        return None


def test_server(host: str, port: typing.Union[str, int], timeOut: int = 4) -> bool:
    try:
        sock = socket.create_connection((host, int(port)), timeOut)
        sock.close()
    except Exception:
        return False
    return True


def find_application(appName: str, extraPath: typing.Optional[str] = None) -> typing.Optional[str]:
    searchPath = os.environ['PATH'].split(os.pathsep)
    if extraPath:
        searchPath += list(extraPath)

    for path in searchPath:
        fileName = os.path.join(path, appName)
        if os.path.isfile(fileName) and (os.stat(fileName).st_mode & stat.S_IXUSR) != 0:
            return fileName
    return None


def get_hostname() -> str:
    '''
    Returns current host name
    In fact, it's a wrapper for socket.gethostname()
    '''
    hostname = socket.gethostname()
    logger.info('Hostname: %s', hostname)
    return hostname


# Queing operations (to be executed before exit)


def register_for_delayed_deletion(filename: str, early: bool = False) -> None:
    '''
    Adds a file to the wait-and-unlink list
    '''
    logger.debug('Added file %s to unlink on %s stage', filename, 'early' if early else 'later')
    _unlinkFiles.append((filename, early))


def unlink_files(early: bool = False) -> None:
    '''
    Removes all wait-and-unlink files
    '''
    logger.debug('Unlinking files on %s stage', 'early' if early else 'later')
    filesToUnlink = list(filter(lambda x: x[1] == early, _unlinkFiles))
    if filesToUnlink:
        logger.debug('Files to unlink: %s', filesToUnlink)
        # Wait 2 seconds before deleting anything on early and 5 on later stages
        time.sleep(1 + 2 * (1 + int(early)))

        for f in filesToUnlink:
            try:
                os.unlink(f[0])
            except Exception as e:
                logger.debug('File %s not deleted: %s', f[0], e)


def add_task_to_wait(task: typing.Any, includeSubprocess: bool = False) -> None:
    logger.debug(
        'Added task %s to wait %s',
        task,
        'with subprocesses' if includeSubprocess else '',
    )
    _tasks_to_Wait.append((task, includeSubprocess))


def waitForTasks() -> None:
    logger.debug('Started to wait %s', _tasks_to_Wait)
    for task, waitForSubp in _tasks_to_Wait:
        logger.debug('Waiting for task %s, subprocess wait: %s', task, waitForSubp)
        try:
            if hasattr(task, 'join'):
                task.join()
            elif hasattr(task, 'wait'):
                task.wait()
            # If wait for spanwed process (look for process with task pid) and we can look for them...
            logger.debug(
                'Psutil: %s, waitForSubp: %s, hasattr: %s',
                psutil,
                waitForSubp,
                hasattr(task, 'pid'),
            )
            if psutil and waitForSubp and hasattr(task, 'pid'):
                subprocesses: list['psutil.Process'] = list(
                    filter(
                        lambda x: x.ppid() == task.pid,  # type x: psutil.Process
                        psutil.process_iter(attrs=('ppid',)),
                    )
                )
                logger.debug('Waiting for subprocesses... %s, %s', task.pid, subprocesses)
                for i in subprocesses:
                    logger.debug('Found %s', i)
                    i.wait()
        except Exception as e:
            logger.error('Waiting for tasks to finish error: %s', e)


def register_execute_before_exit(fnc: typing.Callable[[], None]) -> None:
    logger.debug('Added exec before exit: %s', fnc)
    _execBeforeExit.append(fnc)


def exec_before_exit() -> None:
    logger.debug('Esecuting exec before exit: %s', _execBeforeExit)
    for fnc in _execBeforeExit:
        fnc()


def verify_signature(script: bytes, signature: bytes) -> bool:
    '''
    Verifies with a public key from whom the data came that it was indeed
    signed by their private key
    param: public_key_loc Path to public key
    param: signature String signature to be verified
    return: Boolean. True if the signature is valid; False otherwise.
    '''
    public_key = serialization.load_pem_public_key(data=consts.PUBLIC_KEY, backend=default_backend())

    try:
        public_key.verify(  # type: ignore
            base64.b64decode(signature), script, padding.PKCS1v15(), hashes.SHA256()  # type: ignore
        )
    except Exception:  # InvalidSignature
        return False

    # If no exception, the script was fine...
    return True


def get_cacerts_file() -> typing.Optional[str]:
    # First, try certifi...

    # If environment contains CERTIFICATE_BUNDLE_PATH, use it
    if 'CERTIFICATE_BUNDLE_PATH' in os.environ:
        return os.environ['CERTIFICATE_BUNDLE_PATH']

    try:
        if os.path.exists(certifi.where()):
            return certifi.where()
    except Exception:
        pass

    logger.info('Certifi file does not exists: %s', certifi.where())

    # Check if "standard" paths are valid for linux systems
    if 'linux' in sys.platform:
        for path in (
            '/etc/pki/tls/certs/ca-bundle.crt',
            '/etc/ssl/certs/ca-certificates.crt',
            '/etc/ssl/ca-bundle.pem',
        ):
            if os.path.exists(path):
                logger.info('Found certifi path: %s', path)
                return path

    return None


def is_mac_os() -> bool:
    return 'darwin' in sys.platform


# old compat names, to ensure compatibility with old code
# Basically, this will be here until v5.0. On 4.5 (or even later) Broker plugins will update
# (making them imcompatible with 3.x versions)
addTaskToWait = add_task_to_wait
saveTempFile = save_temp_file
readTempFile = read_temp_file
testServer = test_server
findApp = find_application
getHostName = get_hostname
addFileToUnlink = register_for_delayed_deletion
unlinkFiles = unlink_files
isMac = is_mac_os
getCaCertsFile = get_cacerts_file
verifySignature = verify_signature
execBeforeExit = exec_before_exit
addExecBeforeExit = register_execute_before_exit
