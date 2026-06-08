# Variables de Entorno de UDS Client

Este documento detalla las variables de entorno soportadas por el cliente UDS, clasificadas por su funcionalidad.

---

## 📹 Redirección de Webcam

Estas variables permiten diagnosticar y configurar el comportamiento de la redirección de cámara web en sesiones RDP:

### `UDS_WEBCAM_FORMAT`
* **Descripción**: Fuerza el formato de codificación utilizado para transmitir los frames de la webcam al servidor.
* **Valores posibles**:
  * `h264` o `1`: Fuerza el uso del codificador H.264 (OpenH264).
  * `mjpeg` o `2`: Fuerza la codificación en MJPEG utilizando TurboJPEG.
  * `yuy2` o `3`: Fuerza el formato YUY2 sin compresión adicional.
  * `raw` o `0`: Envía frames raw sin compresión.

### `UDS_WEBCAM_MOCK`
* **Descripción**: Si está definida, simula una webcam de prueba generando un patrón en movimiento (un cuadrado rebotando con gradientes de color). Es ideal para pruebas donde no se dispone de una cámara física o para depuración.

### `UDS_WEBCAM_DEVICE`
* **Descripción**: Selecciona un dispositivo de vídeo específico.
* **Valores posibles**:
  * Un número entero (ej. `0`, `1`): El índice del dispositivo en el sistema.
  * Un texto: Busca el primer dispositivo cuyo nombre amigable contenga dicho texto (búsqueda insensible a mayúsculas/minúsculas).
* **Ejemplo**: `UDS_WEBCAM_DEVICE="Logitech"` seleccionará automáticamente la cámara Logitech.

---

## 📝 Sistema de Logs

Permiten configurar la verbosidad y la ruta de almacenamiento de los ficheros de log generados por la aplicación.

### `UDSLAUNCHER_LOG_LEVEL`
* **Descripción**: Nivel mínimo de trazas a registrar para el proceso principal del Launcher.
* **Valores**: `trace`, `debug`, `info`, `warn`, `error`.
* **Por defecto**: `debug` en compilaciones de desarrollo (`debug`), `info` en producción (`release`).

### `UDSLAUNCHER_LOG_PATH`
* **Descripción**: Ruta del directorio donde se escribirá el archivo de log.
* **Por defecto**: Directorio temporal del sistema (ej. `%TEMP%` en Windows o `/tmp` en Unix).

### `UDSLAUNCHER_LOG_USE_DATETIME`
* **Descripción**: Si se establece en `true`, incluye la fecha, la hora y el nombre del host en el nombre del fichero de log para evitar sobrescribir logs anteriores.
* **Valores**: `true`, `false`.
* **Por defecto**: `false`.

### Archivos de Test (`UDSLAUNCHER-TESTS_LOG_...`)
El set homólogo de variables controla el comportamiento de los logs de pruebas unitarias/integración:
* `UDSLAUNCHER-TESTS_LOG_LEVEL`
* `UDSLAUNCHER-TESTS_LOG_PATH`
* `UDSLAUNCHER-TESTS_LOG_USE_DATETIME`

---

## 🛠️ Depuración y Desarrollo

### `UDS_DEBUG_ARGS`
* **Descripción**: Disponible únicamente en compilaciones de depuración (`debug_assertions`). Permite inyectar los argumentos de inicio (como la URL del protocolo `udssv2://...`) a través del entorno en lugar de la línea de comandos, facilitando el debugging desde el IDE.
