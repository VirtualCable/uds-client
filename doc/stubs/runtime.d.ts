declare module "runtime" {
  export namespace Utils {
    function expandVars(input: string): string;
    function cryptProtectData(input: string): string;
    function writeHkcu(key: string, value_name: string, value_data: string): void;
    function writeHkcuDword(key: string, value_name: string, value_data: number): void;
    function readHkcu(key: string, value_name: string): string | undefined;
    function readHklm(key: string, value_name: string): string;
    function testServer(host: string, port: number, timeout_ms: number): Promise<boolean>;
    function sleep(milliseconds: number): Promise<void>;
  }

  export namespace File {
    function createTempFile(folder?: string, content?: string, extension?: string): string;
    function read(path: string): string;
    function write(path: string, content: string): boolean;
    function exists(path: string): boolean;
    function isExecutable(path: string): boolean;
    function isDirectory(path: string): boolean;
    function getTempDirectory(): string;
    function getHomeDirectory(): string;
  }

  export namespace Logger {
    function trace(msg: string): void;
    function debug(msg: string): void;
    function info(msg: string): void;
    function warn(msg: string): void;
    function error(msg: string): void;
  }

  export namespace Process {
    function findExecutable(app_name: string, extra_path?: string[]): string | null;
    function launch(app_path: string, app_args: string[]): number;
    function isRunning(process_id: number): boolean;
    function kill(process_id: number): void;
    function wait(process_id: number): Promise<void>;
    function waitTimeout(process_id: number, timeout_ms: number): Promise<boolean>;
    function launchAndWait(app_path: string, app_args: string[], timeout_ms?: number): Promise<{stdout: string, stderr: string}>;
    function sleep(milliseconds: number): Promise<void>;
  }

  export namespace Tasks {
    function addEarlyUnlinkableFile(file_path: string): void;
    function addLateUnlinkableFile(file_path: string): void;
    function addWaitableApp(task_handle: number): void;
    function startTunnel(params: { addr: string, port: number, ticket: string, startup_time_ms?: number, check_certificate?: boolean, local_port?: number, keep_listening_after_timeout?: boolean, enable_ipv6?: boolean, crypto_params?: { key_send: Uint8Array | number[], key_receive: Uint8Array | number[], nonce_send: Uint8Array | number[], nonce_receive: Uint8Array | number[] } }): Promise<{port: number}>;
  }

  export namespace RDP {
    function start(settings: {
      server: string;
      port?: number;
      user?: string;
      password?: string;
      domain?: string;
      verify_cert?: boolean;
      use_nla?: boolean;
      screen_width?: number;
      screen_height?: number;
      clipboard_redirection?: boolean;
      audio_redirection?: boolean;
      microphone_redirection?: boolean;
      printer_redirection?: boolean;
      drives_to_redirect?: string[];
      sound_latency_threshold?: number;
    }): void;
  }
}
