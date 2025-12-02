# JavaScript Runtime API

This document describes the JavaScript modules and functions available in the runtime environment.

## Module Overview

| Module   | Description | Functions |
|----------|-------------|-----------|
| Utils    | Utility functions for environment variables, registry (Windows), encryption, and network testing | 8 |
| File     | File operations, temporary files, and directory access | 8 |
| Logger   | Logging functions at different levels | 5 |
| Process  | Executable finding, process launching, and management | 8 |
| Tasks    | Task management, cleanup files, and tunnel connections | 4 |
| RDP      | RDP connection management | 1 |

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

**Returns:** undefined - Currently returns undefined; may be a bug.

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

### sleep (async)

Waits for a specified number of milliseconds.

**Parameters:**
- `milliseconds` (number): The number of milliseconds to wait.

**Returns:** undefined

### Examples

```javascript
// Wait for 1 second
await Utils.sleep(1000);

// Test server connectivity
const isOpen = await Utils.testServer("google.com", 80, 1000);

// Expand environment variables
const expanded = Utils.expandVars("%USERPROFILE%/file.txt");

// Read registry (Windows only)
const value = Utils.readHklm("SOFTWARE\\Microsoft\\Windows\\CurrentVersion", "ProgramFilesDir");
```

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

### Examples

```javascript
// Create a temporary file
const tempPath = File.createTempFile(null, "Hello World", "txt");

// Read file content
const content = File.read(tempPath);

// Write new content
File.write(tempPath, "New content");

// Check if file exists
const exists = File.exists(tempPath);

// Get temp directory
const tempDir = File.getTempDirectory();
```

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

### Examples

```javascript
Logger.trace("This is a trace message");
Logger.debug("This is a debug message");
Logger.info("This is an info message");
Logger.warn("This is a warning message");
Logger.error("This is an error message");
```

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

### sleep (async)

Waits for a specified number of milliseconds.

**Parameters:**
- `milliseconds` (number): The number of milliseconds to wait.

**Returns:** undefined

### Examples

```javascript
// Find an executable
const path = Process.findExecutable("notepad.exe");

// Launch an application
const pid = Process.launch("notepad.exe", []);

// Check if running
const isRunning = Process.isRunning(pid);

// Wait for process
await Process.wait(pid);

// Launch and wait for output
const result = await Process.launchAndWait("echo", ["Hello"], 5000);
console.log(result.stdout); // "Hello"

// Sleep for 1 second
await Process.sleep(1000);
```

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
- `startup_time_ms` (number, optional): Startup timeout in milliseconds (default: 0).
- `check_certificate` (boolean, optional): Whether to check certificates (default: true).
- `local_port` (number, optional): The local port to bind to.
- `keep_listening_after_timeout` (boolean, optional): Whether to keep listening after timeout (default: false).
- `enable_ipv6` (boolean, optional): Whether to enable IPv6 (default: false).

**Returns:** object - An object containing the assigned port: `{port: number}`.

### Examples

```javascript
// Add files for cleanup
Tasks.addEarlyUnlinkableFile("temp1.txt");
Tasks.addLateUnlinkableFile("temp2.txt");

// Add waitable app
Tasks.addWaitableApp(12345);

// Start tunnel
const tunnel = await Tasks.startTunnel("example.com", 443, "ticket123", 5000, true);
console.log("Tunnel port:", tunnel.port);
```

## RDP Module

The RDP module provides functions for managing RDP connections.

### start

Starts an RDP connection with the specified settings.

**Parameters:**
- `settings` (object): An object containing RDP connection settings with the following optional properties:
  - `server` (string): The RDP server address (required).
  - `port` (number, optional): The RDP server port (default: 3389).
  - `user` (string, optional): The username for authentication.
  - `password` (string, optional): The password for authentication.
  - `domain` (string, optional): The domain for authentication.
  - `verify_cert` (boolean, optional): Whether to verify the server certificate (default: true).
  - `use_nla` (boolean, optional): Whether to use Network Level Authentication (default: true).
  - `screen_width` (number, optional): The screen width (0 for full screen).
  - `screen_height` (number, optional): The screen height (0 for full screen).
  - `drives_to_redirect` (array of strings, optional): List of drive letters to redirect.

**Returns:** undefined

### Examples

```javascript
// Start RDP connection with basic settings
RDP.start({
    server: "192.168.1.100",
    port: 3389,
    user: "username",
    password: "password",
    domain: "DOMAIN",
    verify_cert: true,
    use_nla: true,
    screen_width: 1920,
    screen_height: 1080,
    drives_to_redirect: ["C", "D"]
});
```

## Complete Function Reference

| Module  | Function                  | Parameters | Description |
|---------|---------------------------|------------|-------------|
| Utils   | expandVars               | input: string | Expands environment variables in string |
| Utils   | cryptProtectData         | input: string | Encrypts data using Windows CryptProtectData (Windows only) |
| Utils   | writeHkcu                | key: string, value_name: string, value_data: string | Writes string to HKCU registry (Windows only) |
| Utils   | writeHkcuDword           | key: string, value_name: string, value_data: number | Writes DWORD to HKCU registry (Windows only) |
| Utils   | readHkcu                 | key: string, value_name: string | Attempts to read string from HKCU registry (Windows only) - currently returns undefined |
| Utils   | readHklm                 | key: string, value_name: string | Reads string from HKLM registry (Windows only) |
| Utils   | testServer (async)       | host: string, port: number, timeout_ms: number | Tests server connectivity |
| Utils   | sleep (async)            | milliseconds: number | Waits for a specified number of milliseconds |
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
| Process | sleep (async)            | milliseconds: number | Waits for a specified number of milliseconds |
| Tasks   | addEarlyUnlinkableFile   | file_path: string | Adds file for early cleanup |
| Tasks   | addLateUnlinkableFile    | file_path: string | Adds file for late cleanup |
| Tasks   | addWaitableApp           | task_handle: number | Adds waitable application |
| Tasks   | startTunnel (async)      | addr: string, port: number, ticket: string, startup_time_ms?: number, check_certificate?: boolean, local_port?: number, keep_listening_after_timeout?: boolean, enable_ipv6?: boolean | Starts tunnel connection |
| RDP     | start            | settings: object | Starts RDP connection |