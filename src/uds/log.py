# -*- coding: utf-8 -*-
#
# Copyright (c) 2014-2024 Virtual Cable S.L.U.
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
import io
import logging
import os
import os.path
import platform
import sys
import typing

from . import consts


# Local variables
_log_stream = io.StringIO()  # Used to store log for remote log
_log_ticket = ''  # Used to store ticket for remote log


def _platform_debug_info(logger: 'logging.Logger') -> None:
    from . import ui  # To incldue qt version

    # Include as much as platform info as possible
    logger.debug('Platform info:')
    logger.debug('  UDSClient version: %s', consts.VERSION)
    logger.debug('  Platform: %s', platform.platform())
    logger.debug('  Node: %s', platform.node())
    logger.debug('  System: %s', platform.system())
    logger.debug('  Release: %s', platform.release())
    logger.debug('  Version: %s', platform.version())
    logger.debug('  Machine: %s', platform.machine())
    logger.debug('  Processor: %s', platform.processor())
    logger.debug('  Architecture: %s', platform.architecture())
    logger.debug('  Python version: %s', platform.python_version())
    logger.debug('  Python implementation: %s', platform.python_implementation())
    logger.debug('  Python compiler: %s', platform.python_compiler())
    logger.debug('  Python build: %s', platform.python_build())
    # Also environment variables and any useful info
    logger.debug('Qt framework: %s', ui.QT_VERSION)
    logger.debug('Log level set to DEBUG')
    logger.debug('Environment variables:')
    for k, v in os.environ.items():
        logger.debug('  %s=%s', k, v)

    # useful info for debugging
    logger.debug('Python path: %s', sys.path)
    logger.debug('Python executable: %s', sys.executable)
    logger.debug('Python version: %s', sys.version)
    logger.debug('Python version info: %s', sys.version_info)
    logger.debug('Python prefix: %s', sys.prefix)
    logger.debug('Python base prefix: %s', sys.base_prefix)
    logger.debug('Python executable: %s', sys.executable)
    logger.debug('Python argv: %s', sys.argv)
    logger.debug('Python modules path: %s', sys.path)
    logger.debug('Python modules importer cache path: %s', sys.path_importer_cache)
    logger.debug('Python modules hooks path: %s', sys.path_hooks)
    logger.debug('Python modules meta path: %s', sys.meta_path)


def _init() -> 'logging.Logger':
    try:
        logging.basicConfig(
            filename=consts.LOGFILE,
            filemode='a',
            format='%(levelname)s %(asctime)s %(message)s',
            level=consts.LOGLEVEL,
        )
    except Exception:
        logging.basicConfig(format='%(levelname)s %(asctime)s %(message)s', level=consts.LOGLEVEL)

    logger = logging.getLogger('udsclient')

    if consts.DEBUG:
        _platform_debug_info(logger)

    return logger


def init_remote_log(log_data: typing.Dict[str, typing.Any]) -> None:
    try:
        if log_data.get('ticket') is not None:
            log_level = log_data.get('level', logging.INFO)
            stream_handler = logging.StreamHandler(_log_stream)
            stream_handler.setLevel(log_level)

        _log_stream.truncate(0)
        # Repeat platform info for remote (will be duplicated locally if debug is enabled, but that's ok)
        _platform_debug_info(logger)

        _log_ticket = log_data.get('ticket', '')
    except Exception as e:
        logger.error('Error setting log level as requested to %s: %s', log_data.get('level'), e)


def get_remote_log() -> typing.Tuple[str, str]:
    return _log_ticket, _log_stream.getvalue()


# Initialize logger
logger = _init()
