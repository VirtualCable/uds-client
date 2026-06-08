# Variables de Entorno de UDS Client

Este documento detalla las variables de entorno soportadas por el cliente UDS, clasificadas por su funcionalidad.

---

## 📹 Redirección de Webcam

Estas variables permiten diagnosticar y configurar el comportamiento de la redirección de cámara web en sesiones RDP:

### `UDSLAUNCHER_CAM_FORMAT`
* **Descripción**: Fuerza el formato de codificación utilizado para transmitir los frames de la webcam al servidor.
* **Valores posibles**:
  * `h264` o `1`: Fuerza el uso del codificador H.264 (OpenH264).
  * `mjpeg` o `2`: Fuerza la codificación en MJPEG utilizando TurboJPEG.
  * `yuy2` o `3`: Fuerza el formato YUY2 sin compresión adicional.
  * `raw` o `0`: Envía frames raw sin compresión.

### `UDSLAUNCHER_CAM_MOCK`
* **Descripción**: Si está definida, simula una webcam de prueba generando un patrón en movimiento (un cuadrado rebotando con gradientes de color). Es ideal para pruebas donde no se dispone de una cámara física o para depuración.

### `UDSLAUNCHER_CAM_DEVICE`
* **Descripción**: Selecciona un dispositivo de vídeo específico.
* **Valores posibles**:
  * Un número entero (ej. `0`, `1`): El índice del dispositivo en el sistema.
  * Un texto: Busca el primer dispositivo cuyo nombre amigable contenga dicho texto (búsqueda insensible a mayúsculas/minúsculas).
* **Ejemplo**: `UDSLAUNCHER_CAM_DEVICE="Logitech"` seleccionará automáticamente la cámara Logitech.

---

## ⚙️ Limitación Dinámica de Parámetros

### `UDSLAUNCHER_LIMITS`
* **Descripción**: Permite sobreescribir y **disminuir/reducir** los límites máximos de rendimiento y calidad para la redirección de la cámara web. **Nota importante**: Solo permite reducir los límites impuestos por el administrador en la sesión (para solucionar problemas de ancho de banda o consumo de CPU local), **nunca ampliarlos**.
* **Formato**: `ancho,alto,fps,calidad` (valores enteros separados por comas). 
* **Reglas**:
  * Los campos son posicionales y opcionales. Un valor vacío (ej. `,` o espacios) indica mantener el valor original de la sesión.
  * Si se proporciona una resolución, ambos valores (ancho y alto) deben indicarse.
* **Ejemplos**:
  * `640,480,10,50`: Reduce el tamaño máximo capturado a 640x480, limita los FPS a 10 y reduce la calidad de compresión a un máximo de 50.
  * `,,15,`: Deja la resolución original intacta, pero limita los FPS de captura a un máximo de 15.
  * `1280,720,,`: Limita la resolución máxima capturada a 1280x720, manteniendo los FPS y calidad configurados en la sesión.

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
