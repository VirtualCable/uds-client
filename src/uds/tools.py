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

from uds import types

try:
    import psutil

    def process_iter(*args: typing.Any, **kwargs: typing.Any) -> typing.Any:
        return psutil.process_iter(*args, **kwargs)

except ImportError:

    def process_iter(*args: typing.Any, **kwargs: typing.Any) -> typing.Any:
        return []


from . import consts
from .log import logger


# Global variables is fine, no more than one thread will be running
# at the same time for the same process, so no need to lock
_unlink_files: typing.List[types.RemovableFile] = []
_awaitable_tasks: typing.List[types.AwaitableTask] = []
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


def test_server(host: str, port: typing.Union[str, int], timeout: int = 4) -> bool:
    try:
        sock = socket.create_connection((host, int(port)), timeout)
        sock.close()
    except Exception:
        return False
    return True


def find_application(application_name: str, extra_path: typing.Optional[str] = None) -> typing.Optional[str]:
    searchPath = os.environ['PATH'].split(os.pathsep)
    if extra_path:
        searchPath += list(extra_path)

    for path in searchPath:
        fileName = os.path.join(path, application_name)
        if os.path.isfile(fileName) and (os.stat(fileName).st_mode & stat.S_IXUSR) != 0:
            return fileName
    return None


def gethostname() -> str:
    '''
    Returns current host name
    In fact, it's a wrapper for socket.gethostname()
    '''
    hostname = socket.gethostname()
    logger.info('Hostname: %s', hostname)
    return hostname


# Queing operations (to be executed before exit)


def register_for_delayed_deletion(filename: str, early_stage: bool = False) -> None:
    '''
    Adds a file to the wait-and-unlink list
    '''
    logger.debug('Added file %s to unlink on %s stage', filename, 'early' if early_stage else 'later')
    _unlink_files.append(types.RemovableFile(filename, early_stage))


def unlink_files(early_stage: bool = False) -> None:
    '''
    Removes all wait-and-unlink files
    '''
    logger.debug('Unlinking files on %s stage', 'early' if early_stage else 'later')
    files_to_unlink = list(filter(lambda x: x.early_stage == early_stage, _unlink_files))
    if files_to_unlink:
        logger.debug('Files to unlink: %s', files_to_unlink)
        # Wait 2 seconds before deleting anything on early and 5 on later stages
        time.sleep(1 + 2 * (1 + int(early_stage)))

        for f in files_to_unlink:
            try:
                os.unlink(f.path)
            except Exception as e:
                logger.debug('File %s not deleted: %s', f[0], e)

    # Remove all processed files from list
    _unlink_files[:] = list(filter(lambda x: x.early_stage != early_stage, _unlink_files))


def add_task_to_wait(task: typing.Any, wait_subprocesses: bool = False) -> None:
    logger.debug(
        'Added task %s to wait %s',
        task,
        'with subprocesses' if wait_subprocesses else '',
    )
    _awaitable_tasks.append(types.AwaitableTask(task, wait_subprocesses))


def wait_for_tasks() -> None:
    logger.debug('Started to wait %s', _awaitable_tasks)
    for awaitable_task in _awaitable_tasks:
        logger.debug(
            'Waiting for task %s, subprocess wait: %s', awaitable_task.task, awaitable_task.wait_subprocesses
        )
        try:
            if hasattr(awaitable_task.task, 'join'):
                awaitable_task.task.join()
            elif hasattr(awaitable_task.task, 'wait'):
                awaitable_task.task.wait()
            # If wait for spanwed process (look for process with task pid) and we can look for them...
            if awaitable_task.wait_subprocesses and hasattr(awaitable_task.task, 'pid'):
                subprocesses: list['psutil.Process'] = list(
                    filter(
                        lambda x: x.ppid() == awaitable_task.task.pid,  # type x: psutil.Process
                        process_iter(attrs=('ppid',)),
                    )
                )
                logger.debug('Waiting for subprocesses... %s, %s', awaitable_task.task.pid, subprocesses)
                for i in subprocesses:
                    logger.debug('Found %s', i)
                    i.wait()
        except Exception as e:
            logger.error('Waiting for tasks to finish error: %s', e)

    # Empty the list
    _awaitable_tasks[:] = typing.cast(list[types.AwaitableTask], [])


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
# waitForTasks = wait_for_tasks  # Not used on scripts
saveTempFile = save_temp_file
readTempFile = read_temp_file
testServer = test_server
findApp = find_application
# getHostName = get_hostname  # Not used on scripts
addFileToUnlink = register_for_delayed_deletion
# unlinkFiles = unlink_files  # Not used on existing scripts
# isMac = is_mac_os  # Not used on scripts  # Not used on existing scripts
# getCaCertsFile = get_cacerts_file  # Not used on scripts
# verifySignature = verify_signature  # Not used on scripts
# execBeforeExit = exec_before_exit  # Not used on scripts
# addExecBeforeExit = register_execute_before_exit  # Not used on existing scripts
