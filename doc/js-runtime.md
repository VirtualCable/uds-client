# JavaScript Runtime API

This document describes the JavaScript modules and functions available in the runtime environment.

## Module Overview

| Module   | Description | Functions |
|----------|-------------|-----------|
| Utils    | Utility functions for environment variables, registry (Windows), encryption, and network testing | 7 |
| File     | File operations, temporary files, and directory access | 8 |
| Logger   | Logging functions at different levels | 5 |
| Process  | Executable finding, process launching, and management | 7 |
| Tasks    | Task management, cleanup files, and tunnel connections | 4 |

## Utils Module

The Utils module provides utility functions for environment variable expansion, registry operations (Windows only), encryption, and network testing.

### expandVars

Expands environment variables in the input string. On Windows, uses `%VAR%` syntax; on Unix-like systems, uses `$VAR` or `${VAR}`.

**Parameters:**
- `input` (string): The string containing variables to expand.

**Returns:** string - The expanded string.

### cryptProtectData (Windows only)

Encrypts the input data using Windows CryptProtectData and returns it as a base64-encoded string.

**Parameters:**
- `input` (string): The data to encrypt.

**Returns:** string - The encrypted data as base64.

### writeHkcu (Windows only)

Writes a string value to the HKCU (HKEY_CURRENT_USER) registry key.

**Parameters:**
- `key` (string): The registry key path.
- `value_name` (string): The value name.
- `value_data` (string): The string value to write.

### writeHkcuDword (Windows only)

Writes a DWORD (32-bit unsigned integer) value to the HKCU registry key.

**Parameters:**
- `key` (string): The registry key path.
- `value_name` (string): The value name.
- `value_data` (number): The DWORD value to write.

### readHkcu (Windows only)

Reads a string value from the HKCU registry key. Note: Currently returns undefined; may be a bug.

**Parameters:**
- `key` (string): The registry key path.
- `value_name` (string): The value name.

### readHklm (Windows only)

Reads a string value from the HKLM (HKEY_LOCAL_MACHINE) registry key.

**Parameters:**
- `key` (string): The registry key path.
- `value_name` (string): The value name.

**Returns:** string - The read value.

### testServer (async)

Tests connectivity to a server by attempting to establish a TCP connection.

**Parameters:**
- `host` (string): The hostname or IP address.
- `port` (number): The port number.
- `timeout_ms` (number): Timeout in milliseconds (0 defaults to 500ms).

**Returns:** boolean - True if connection successful, false otherwise.

## File Module

The File module provides functions for file operations, temporary file creation, and directory access.

### createTempFile

Creates a temporary file with optional content and extension in the specified folder or system temp directory.

**Parameters:**
- `folder` (string, optional): The folder to create the file in. If not provided, uses system temp directory.
- `content` (string, optional): The content to write to the file.
- `extension` (string, optional): The file extension (defaults to "tmp").

**Returns:** string - The path to the created file.

### read

Reads the entire content of a file as a string.

**Parameters:**
- `path` (string): The file path to read.

**Returns:** string - The file content.

### write

Writes content to a file, creating or overwriting it.

**Parameters:**
- `path` (string): The file path to write to.
- `content` (string): The content to write.

**Returns:** boolean - True if successful.

### exists

Checks if a file or directory exists at the given path.

**Parameters:**
- `path` (string): The path to check.

**Returns:** boolean - True if the path exists.

### isExecutable

Checks if the file at the given path is executable.

**Parameters:**
- `path` (string): The file path to check.

**Returns:** boolean - True if the file is executable.

### isDirectory

Checks if the path is a directory.

**Parameters:**
- `path` (string): The path to check.

**Returns:** boolean - True if the path is a directory.

### getTempDirectory

Gets the system temporary directory path.

**Parameters:** None

**Returns:** string - The temp directory path.

### getHomeDirectory

Gets the user's home directory path.

**Parameters:** None

**Returns:** string - The home directory path.

## Logger Module

The Logger module provides logging functions at different levels.

### trace

Logs a message at trace level.

**Parameters:**
- `msg` (string): The message to log.

### debug

Logs a message at debug level.

**Parameters:**
- `msg` (string): The message to log.

### info

Logs a message at info level.

**Parameters:**
- `msg` (string): The message to log.

### warn

Logs a message at warn level.

**Parameters:**
- `msg` (string): The message to log.

### error

Logs a message at error level.

**Parameters:**
- `msg` (string): The message to log.

## Process Module

The Process module provides functions for finding executables, launching processes, and managing running processes.

### findExecutable

Searches for an executable in the system PATH and additional provided paths.

**Parameters:**
- `app_name` (string): The name of the executable to find.
- `extra_path` (array of strings): Additional paths to search in.

**Returns:** string or null - The full path to the executable if found, null otherwise.

### launch

Launches an application in the background and returns its process ID.

**Parameters:**
- `app_path` (string): The path to the executable.
- `app_args` (array of strings): Command-line arguments for the application.

**Returns:** number - The process ID of the launched application.

### isRunning

Checks if a process with the given ID is currently running.

**Parameters:**
- `process_id` (number): The process ID to check.

**Returns:** boolean - True if the process is running.

### kill

Terminates a process with the given ID.

**Parameters:**
- `process_id` (number): The process ID to terminate.

### wait

Waits indefinitely for a process to finish.

**Parameters:**
- `process_id` (number): The process ID to wait for.

### waitTimeout

Waits for a process to finish with a timeout.

**Parameters:**
- `process_id` (number): The process ID to wait for.
- `timeout_ms` (number): Timeout in milliseconds.

**Returns:** boolean - True if the timeout was triggered, false if the process finished.

### launchAndWait (async)

Launches an application, waits for it to finish, and returns its output.

**Parameters:**
- `app_path` (string): The path to the executable.
- `app_args` (array of strings): Command-line arguments for the application.
- `timeout_ms` (number, optional): Timeout in milliseconds (default: 30000).

**Returns:** object - An object containing stdout and stderr: `{stdout: string, stderr: string}`.

## Tasks Module

The Tasks module provides functions for managing tasks, files to be cleaned up, and tunnel connections.

### addEarlyUnlinkableFile

Adds a file to the list of files to be unlinked early in the process lifecycle.

**Parameters:**
- `file_path` (string): The path of the file to add.

### addLateUnlinkableFile

Adds a file to the list of files to be unlinked late in the process lifecycle.

**Parameters:**
- `file_path` (string): The path of the file to add.

### addWaitableApp

Adds an application handle to the list of waitable applications.

**Parameters:**
- `task_handle` (number): The application handle (process ID).

### startTunnel (async)

Starts a tunnel connection.

**Parameters:**
- `addr` (string): The address to connect to.
- `port` (number): The port number.
- `ticket` (string): The connection ticket.
- `listen_timeout_ms` (number, optional): Listen timeout in milliseconds (default: 0).
- `local_port` (number, optional): The local port to bind to.
- `check_certificate` (boolean, optional): Whether to check certificates (default: true).
- `keep_listening_after_timeout` (boolean, optional): Whether to keep listening after timeout (default: false).
- `enable_ipv6` (boolean, optional): Whether to enable IPv6 (default: false).

**Returns:** object - An object containing the assigned port: `{port: number}`.

## Complete Function Reference

| Module  | Function                  | Parameters | Description |
|---------|---------------------------|------------|-------------|
| Utils   | expandVars               | input: string | Expands environment variables in string |
| Utils   | cryptProtectData         | input: string | Encrypts data using Windows CryptProtectData (Windows only) |
| Utils   | writeHkcu                | key: string, value_name: string, value_data: string | Writes string to HKCU registry (Windows only) |
| Utils   | writeHkcuDword           | key: string, value_name: string, value_data: number | Writes DWORD to HKCU registry (Windows only) |
| Utils   | readHkcu                 | key: string, value_name: string | Reads string from HKCU registry (Windows only) |
| Utils   | readHklm                 | key: string, value_name: string | Reads string from HKLM registry (Windows only) |
| Utils   | testServer (async)       | host: string, port: number, timeout_ms: number | Tests server connectivity |
| File    | createTempFile           | folder?: string, content?: string, extension?: string | Creates temporary file |
| File    | read                     | path: string | Reads file content |
| File    | write                    | path: string, content: string | Writes content to file |
| File    | exists                   | path: string | Checks if path exists |
| File    | isExecutable             | path: string | Checks if file is executable |
| File    | isDirectory              | path: string | Checks if path is a directory |
| File    | getTempDirectory         | - | Gets temp directory path |
| File    | getHomeDirectory         | - | Gets home directory path |
| Logger  | trace                    | msg: string | Logs trace message |
| Logger  | debug                    | msg: string | Logs debug message |
| Logger  | info                     | msg: string | Logs info message |
| Logger  | warn                     | msg: string | Logs warn message |
| Logger  | error                    | msg: string | Logs error message |
| Process | findExecutable           | app_name: string, extra_path: string[] | Finds executable path |
| Process | launch                   | app_path: string, app_args: string[] | Launches application |
| Process | isRunning                | process_id: number | Checks if process is running |
| Process | kill                     | process_id: number | Terminates process |
| Process | wait                     | process_id: number | Waits for process to finish |
| Process | waitTimeout              | process_id: number, timeout_ms: number | Waits for process with timeout |
| Process | launchAndWait (async)    | app_path: string, app_args: string[], timeout_ms?: number | Launches application and waits for completion |
| Tasks   | addEarlyUnlinkableFile   | file_path: string | Adds file for early cleanup |
| Tasks   | addLateUnlinkableFile    | file_path: string | Adds file for late cleanup |
| Tasks   | addWaitableApp           | task_handle: number | Adds waitable application |
| Tasks   | startTunnel (async)      | addr: string, port: number, ticket: string, listen_timeout_ms?: number, local_port?: number, check_certificate?: boolean, keep_listening_after_timeout?: boolean, enable_ipv6?: boolean | Starts tunnel connection |