# Printer Redirection Architecture & Plan — uds-client

> **Date:** 2026-10-25  
> **Status:** Architecture Proposal / Technical Roadmap  
> **Target Module:** `uds-client` (`crates/channels/src/printer/`)  
> **Companion Document in freerdp-integration:** `docs/printer-redirection-plan.md`  
> **Underlying Incident Report:** `informe-impresion-rdp-integrado-windows.md`

---

## 1. Motivation and Problem Analysis

The technical investigation documented in *`informe-impresion-rdp-integrado-windows.md`* revealed critical flaws in FreeRDP's native printer redirection implementation (`channels/printer/client`):

1. **Windows Crash (`__fastfail` 0xC0000409)**:
   * When triggering print jobs from the remote session, `launcher.exe` terminates abruptly with zero log output.
   * Root cause: in `channels/printer/client/win/printer_win.c`, `printer_win_get_printjob_name` calls `swprintf_s(NULL, 0, ...)`. In the Microsoft C Runtime (CRT), passing a `NULL` buffer to secure `_s` functions triggers the invalid parameter handler (`_invoke_watson`), causing an immediate `__fastfail` (exit code `0xC0000409`).
2. **Linux Hang / Dropped Jobs**:
   * FreeRDP's native CUPS backend initiates `Create-Job` with CUPS (`/var/log/cups/access_log`), but jobs remain stuck in `pending-held / job-incoming` with 0 KB transferred and never complete, stalling or terminating the launcher after 30 seconds.
3. **macOS Incompatibilities**:
   * Suffers from similar C socket and stream termination problems within the CUPS C addin.

### Why Patching FreeRDP Is Unviable
* In Windows, a patch would only work if recompiling our own bundled binary, but becomes maintenance debt with every FreeRDP version bump.
* In Linux, `uds-client` packages (e.g., Debian/Ubuntu `.deb` packages) dynamically link against the **system distribution package** (`libfreerdp3` from Debian/Ubuntu). We **cannot patch the distribution's system packages**.
* Moving printer redirection into **our own Rust domain** guarantees total control, zero crashes across all operating systems, and independence from upstream FreeRDP release cycles.

---

## 2. Architecture of `crates/channels/src/printer/`

```text
crates/channels/src/printer/
├── mod.rs                // Public facade, PrinterHandle struct, PrinterIntegration implementation
├── types.rs              // Data types (jobs, states, printer metadata)
├── windows/              // #[cfg(target_os = "windows")]
│   ├── mod.rs            // Win32 Spooler native backend
│   ├── spooler.rs        // RAII wrapper over winspool.drv (OpenPrinter, StartDoc, etc.)
│   └── enum_printers.rs  // Native enumeration via EnumPrintersW / GetDefaultPrinterW
├── linux/                // #[cfg(target_os = "linux")]
│   ├── mod.rs            // Linux backend (pure-Rust IPP / CUPS client or local spooling)
│   └── spool.rs          // Print stream dispatch (RAW / PostScript / PDF)
└── macos/                // #[cfg(target_os = "macos")]
    ├── mod.rs            // macOS backend (native CUPS / lp spooling)
    └── spool.rs
```

---

## 3. Platform-Specific Implementations

### 3.1. Windows Backend (`windows/`)
Directly communicates with the Windows Print Spooler (`winspool.drv`):

1. **Printer Enumeration (`enum_printers.rs`)**:
   * Uses `GetDefaultPrinterW` for the primary printer, or `EnumPrintersW` (`PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS`, level 2 or 4).
   * Extracts device name, port name, and driver name.
2. **Job Lifecycle (`spooler.rs`)**:
   * `create_job(printer_name)`:
     * Calls `OpenPrinterW(name, &mut h_printer, null)`.
     * Prepares `DOC_INFO_1W`:
       * `pDocName`: `"UDS Print Job <id>"`.
       * `pOutputFile`: `NULL`.
       * `pDatatype`: `"RAW"` (or `"XPS_PASS"` if the server emits an XPS stream via Remote Desktop Easy Print).
     * Invokes `StartDocPrinterW(h_printer, 1, &doc_info)` followed by `StartPagePrinter(h_printer)`.
     * Returns a unique numeric `job_id` stored in an internal thread-safe RAII registry.
   * `write_data(job_id, data)`:
     * Streams incoming chunks directly via `WritePrinter(h_printer, data.as_ptr(), data.len(), &mut bytes_written)`.
     * Handles large multipage print streams efficiently without memory bloat.
   * `close_job(job_id)`:
     * Finalizes via `EndPagePrinter(h_printer)`, `EndDocPrinter(h_printer)`, and `ClosePrinter(h_printer)`.
   * `abort_job(job_id)`:
     * Calls `AbortPrinter(h_printer)` + `ClosePrinter(h_printer)` on session cancel/disconnect.

**Key Windows Benefits:**
* Eliminates all CRT `swprintf_s` crashes.
* Sends raw data directly into the Windows spooler, supporting virtual PDF printers, network printers, and USB printers natively.

---

### 3.2. Linux Backend (`linux/`)
Resolves the 0 KB `pending-held` issue in CUPS:

1. **Discovery & Status**:
   * Queries local CUPS (`/run/cups/cups.sock` or `http://localhost:631`) via pure-Rust IPP (Internet Printing Protocol, RFC 8011) or parsing `lpstat -d -p`.
2. **Stream Buffering and Spooling**:
   * `create_job`: Prepares a scoped temporary file (`NamedTempFile`) or streaming spool buffer.
   * `write_data`: Sequentially buffers data chunks.
   * `close_job`: Dispatches the completed payload to CUPS with an atomic `Print-Job` request (or invokes `lp -d <printer> <file>`), ensuring the job is never submitted with partial or truncated 0 KB content.

---

### 3.3. macOS Backend (`macos/`)
* Interacts with macOS's native CUPS daemon via local Unix domain sockets.
* Enumerates user-configured default printers.
* Streams and submits complete print jobs reliably through IPP.

---

## 4. Integration into `uds-client` GUI and Launcher

### 4.1. Configuration and Policy Check
In `crates/connection/` and `crates/js/src/js_modules/rdp.rs`:
* Inspect the `redirections.printing` setting.
* When enabled:
  ```rust
  let printer_integration = channels::printer::PrinterHandle::new();
  if let Some(printer) = printer_integration {
      rdp_integrations.printer = Some(Arc::new(printer));
  }
  ```

### 4.2. Resource Management & Session Cleanup
* `PrinterHandle` tracks active jobs in a `Mutex<HashMap<u32, JobContext>>`.
* If the RDP session disconnects abruptly, the `Drop` implementation cleanly aborts any in-flight print jobs, closing OS handles and removing temporary spool files.
