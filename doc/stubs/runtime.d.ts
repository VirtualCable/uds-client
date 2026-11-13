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
    function listFolder(path: string): string[];
    function chdir(path: string): boolean;
    function getCwd(): string;
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
    function startTunnel(params: { addr: string, port: number, ticket: string, startup_time_ms?: number, check_certificate?: boolean, local_port?: number, keep_listening_after_timeout?: boolean, enable_ipv6?: boolean, shared_secret?: Uint8Array | number[] }): Promise<{port: number}>;
  }

  /**
   * **⚠️ Do not use `null` for optional fields.**  
   * Omit the key entirely or provide a valid value.  
   * The Rust `TryFromJs` conversion does not accept `null` for `Option<T>` fields.
   */
  export namespace RDP {
    function start(settings: {
      server: string;
      port?: number;
      user?: string;
      password?: string;
      domain?: string;
      screen_width?: number;
      screen_height?: number;
      best_experience?: boolean;
      redirections?: {
        clipboard?: boolean;
        audio?: boolean;
        mic?: boolean;
        printing?: boolean;
        smartcard?: {
          enabled?: boolean;
          /** Emulated card spec: file:<path> | pem:<cert>,<key> | userdefined: */
          emulated?: string;
        };
        drives?: string[];
        webcam?: {
          enabled: boolean;
          quality?: number;
          fps?: number;
          size_limit?: [number, number];
        };
        /** Clamped to 300..1000 when sent. Omit it to let the client derive one. */
        sound_latency_threshold?: number;
      };
      rail?: {
        app: string;
        args?: string;
        working_dir?: string;
        title?: string;
        server_info?: { id: string; token: string };
        behavior?: "compositegdi" | "individualwindows";
      };
      options?: {
        verify_cert?: boolean;
        use_nla?: boolean;
        use_local_scaler?: boolean;
        use_tunnel?: boolean;
      };
    }): void;
    function sign(rdp_string: string, ticket: string): Promise<string>;
  }


  /**
   * UDS Remote Protocol (URP) session. `ticket` is the broker ticket
   * verbatim (48 alphanumeric chars, never hashed client-side) and
   * `shared_secret` is the KEM envelope's secret (`data.shared_secret`).
   * **Do not use `null` for optional fields** — omit the key instead.
   */
  export namespace URP {
    function start(settings: {
      server: string;
      /** Default: 14433 */
      port?: number;
      /** Broker ticket, 48 alphanumeric characters, verbatim. */
      ticket: string;
      /** Session PSK as 64 hex chars (pass `data.shared_secret` through). */
      shared_secret: string;
      /** Interactive credentials; `user` and `password` must come together. */
      user?: string;
      password?: string;
      domain?: string;
      /** Broker-fixed remote resolution; omit both for "not fixed". */
      screen_width?: number;
      screen_height?: number;
      fullscreen?: boolean;
      /** Stretch the remote image in fullscreen; false = centered with black borders. */
      allow_fullscreen_scaling?: boolean;
      /** Cap for the auto fullscreen resolution (default: 1920x1080). */
      max_fullscreen_width?: number;
      max_fullscreen_height?: number;
      redirections?: {
        audio?: boolean;
        mic?: boolean;
        clipboard?: boolean;
        printing?: boolean;
        webcam?: boolean;
        smartcard?: boolean;
        storage?: boolean;
      };
      options?: {
        /** Codec preset preference; omit to let the server ladder decide. */
        preferred_codec?: string;
      };
    }): void;
  }

  export namespace Dialog {
    export type FieldType = "string" | "password" | "choice" | "text" | "label" | "markdown";

    export interface Field {
      name: string;
      label?: string;
      size?: number;
      type: FieldType;
      value?: string;
      choices?: string[];
    }

    export interface Button {
      name: string;
      label: string;
      closeOnClick?: boolean;
      primary?: boolean;
    }

    export interface Spec {
      title: string;
      fields?: Field[];
      buttons?: Button[];
      width?: number;
      height?: number;
    }

    export interface FieldResult {
      name: string;
      type: FieldType;
      value: string;
    }

    export interface ButtonResult {
      name: string;
      label: string;
      pushed: boolean;
    }

    export interface Result {
      fields: FieldResult[];
      buttons: ButtonResult[];
      pushed?: string;
      values: Record<string, string>;
    }

    function show(spec: Spec | string): Promise<Result>;
    function ask(spec: Spec | string): Promise<Result>;
  }
}
