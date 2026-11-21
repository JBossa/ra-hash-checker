# RA Hash Checker: diseño

Fecha: 2026-11-21 · Estado: borrador pendiente de aprobación

## Intención y por qué

Una app de escritorio de código abierto, simple, para una sola cosa: **sueltas el archivo de un juego en la ventana, calcula su hash de RetroAchievements (RA) y te dice si ese hash está registrado en RA**. Si lo está, también te dice para qué juego.

Nace de un caso real. El usuario tiene juegos de GBA, y antes un .bin de PS1, y quiere saber si su copia es una de las que RA acepta, sin línea de comandos ni scripts. Hoy existen RAHasher (solo línea de comandos y sin comparación) y algunas apps más grandes, como OpenROM o RA-Library. Ninguna es "soltar y listo".

## Objetivos

- Soltar uno o varios archivos y ver, para cada uno: consola detectada, hash, y resultado (**registrado en RA**, con nombre del juego y enlace a su página, o **no registrado**).
- Hashes idénticos a los de RA. Se usa la librería oficial rcheevos, no se reprograma.
- Funcionar sin conexión tras la primera descarga de la lista de hashes de cada consola.
- Un único ejecutable pequeño. Windows primero; Mac y Linux deben poder compilarse.

## No-objetivos (v1)

- Renombrar, organizar, parchear o convertir ROMs.
- Archivos CHD, archivos 7z y juegos de 3DS cifrados.
- Cuentas, logros o progreso del usuario: no se muestra nada de su perfil.
- Revisar una biblioteca completa por carpetas (quizá en el futuro).

## Anti-objetivos (la app fracasa si...)

- La interfaz se vuelve complicada: más de una pantalla principal o necesidad de leer instrucciones.
- Da un hash distinto al que calcula RA para el mismo archivo.
- Comparte o incluye una clave de API común, o usa el endpoint `dorequest.php`, que RA reserva para emuladores.
- Sube a algún sitio los archivos o datos del usuario. Solo se envían peticiones de lectura a la API de RA con la clave del propio usuario.

## Restricciones

- **API de RA:** requiere la clave de API web gratuita de cada usuario (`y`), que se obtiene en retroachievements.org → panel de control → Keys. La documentación pide cachear agresivamente y enviar un User-Agent propio. No publica un límite numérico.
- **rcheevos:** C89, MIT, sin dependencias. Solo se compila la parte de hashes.
- **Licencia del proyecto:** MIT, compatible con rcheevos.

## Enfoques considerados

1. **Tauri 2 (Rust) + rcheevos compilado dentro.** **Elegido por el usuario.** Produce un `.exe` pequeño, permite arrastrar y soltar de forma nativa y es multiplataforma. Los hashes son exactos y lee discos grandes desde disco sin cargarlos en memoria. Es más trabajoso de montar.
2. **Python + interfaz simple + rcheevos como DLL.** El código es más fácil de tocar, pero el ejecutable pesa unos 50 MB y la interfaz se ve anticuada.
3. **Reprogramar los hashes a mano.** Es rápido para GBA, pero aumenta el riesgo de errores con cada consola. Choca con el anti-objetivo de "hash distinto".

## Diseño

### Pantalla (una sola)

```
┌───────────────────────────────────────────────┐
│  RA Hash Checker                     ⚙ Ajustes │
│ ┌───────────────────────────────────────────┐ │
│ │      Suelta aquí tus juegos (o haz clic)  │ │
│ └───────────────────────────────────────────┘ │
│  Pokemon Emerald.gba   GBA   1f1c08…  ✅ Registrado │
│      Pokémon Emerald Version  → ver en RA      │
│  Mario Kart.gba        GBA   9a33e1…  ❌ No registrado │
│      ¿Quizás "Mario Kart: Super Circuit"?      │
│      RA acepta: Mario Kart - Super Circuit (USA) │
│  Bloody Roar II.cue    PS1   77defa…  ✅ Registrado │
└───────────────────────────────────────────────┘
```

- **Ajustes**, en un diálogo: usuario de RA, clave de API, botón "Actualizar listas de hashes" y fecha de la última actualización.
- **Primera vez:** si no hay clave, la zona de soltar muestra un aviso con un enlace a la página de claves de RA. Igual se calcula el hash, pero el resultado dice "sin comparar".
- El hash completo se puede copiar con un clic.
- Idioma: español e inglés según el sistema. *(Decisión mía.)*

### Flujo por archivo

1. **Leer:** los `.zip` solo se admiten para cartuchos. Cada ROM que contenga se extrae en memoria y produce su propia fila, usando su nombre interno para detectar la consola. Si el zip contiene un juego en disco (`.cue`, `.bin`, `.iso`, `.gdi`), esa fila muestra "Descomprime el disco y suéltalo de nuevo". *(Decisión mía.)*
2. **Detectar la consola:** `rc_hash_iterate` prueba, en su propio orden, las consolas que sugiere la extensión.
   - Si la extensión es ambigua (`.bin`, `.iso`, `.cue` puede ser PS1, Saturn, Sega CD…), se prueban los candidatos uno a uno. La lista de cada consola se descarga solo cuando le toca a ese candidato, y la búsqueda se detiene en el primero que coincide. Así se descarga lo mínimo. *(Decisión mía.)*
   - Si ninguno coincide, o no hay clave ni caché para comparar, se muestra el hash del primer candidato de rcheevos con su consola y un selector para cambiarla en esa fila.
3. **Calcular el hash:** `rc_hash_generate(hash, console_id, iterator)`.
4. **Comparar:** se busca el MD5 en el mapa `md5 → juego` de esa consola, construido a partir de la caché.
5. **Si coincide:** se muestra el título, el enlace `retroachievements.org/game/<id>` y, si hay conexión, el nombre del archivo registrado (por ejemplo "… (USA, Europe)") vía `API_GetGameHashes`.
6. **Si no coincide:** se buscan en la lista de esa consola títulos parecidos al nombre del archivo (búsqueda difusa). Para el mejor candidato se muestran los nombres de los archivos que RA acepta (`API_GetGameHashes` → campo `Name`), para que el usuario sepa qué versión necesita. *(Decisión mía, pendiente de confirmar.)*

### Datos de RA y caché

- `API_GetGameList.php?i=<consola>&h=1&y=<clave>` devuelve, por consola, todos los juegos que RA conoce con sus hashes. Se omite `f=1` para no dejar fuera los juegos que aún no tienen logros. Se guarda como JSON en la carpeta de datos de la app.
- Estados posibles de un resultado *(decisión mía)*:
  - **✅ Registrado:** el juego tiene logros.
  - **🟡 Registrado, sin logros todavía:** el juego tiene `NumAchievements = 0`.
  - **❌ No registrado.**
  - **⚪ Sin comparar:** no hay clave ni caché.
- La lista de cada consola se descarga la primera vez que se necesita y se renueva sola si tiene más de 7 días, además del botón manual.
- La lista de hashes de cada juego (`API_GetGameHashes`) se pide solo al mostrar un resultado y se guarda en caché.
- La clave de API se guarda en el llavero del sistema (crate `keyring`), no en texto plano. *(Decisión mía.)*
- User-Agent: `RAHashChecker/<versión> (+<url del repo>)`.

### Arquitectura

- **`rcheevos-hash` (Rust, dentro del repo):** compila con el crate `cc` desde `build.rs` los archivos de hash de rcheevos, con un tag fijo vía submódulo git, y define `RC_HASH_NO_ENCRYPTED`. Expone una función segura: `hash_file(path) -> Vec<(console_id, md5)>` y `hash_buffer(name, bytes, console_id)`.
- **Backend de Tauri:** comandos `hash_files`, `lookup`, `refresh_console`, `get/set_api_key`. Usa `reqwest` para la API, `zip` para los `.zip` y `strsim` o similar para la búsqueda difusa.
- **Frontend:** TypeScript + Vite, sin framework pesado; es una sola vista.
- **Pruebas:**
  - Hashes de ROMs de dominio público o generadas (por ejemplo, homebrew de GBA) comparados con RAHasher.
  - Un test que comprueba el caso de PS1: `77defab4cedf9f82c4251a474d55cbee`, con la imagen local del usuario, fuera de CI.
  - Las respuestas de la API, simuladas en las pruebas.

### Consolas en v1

Todas las que rcheevos soporta, salvo CHD y 3DS cifrado. Se prueban explícitamente GBA, GB/GBC, NES, SNES, Genesis, N64 y PS1 (cue/bin).

## Queda a criterio de quien construya

- Nombre definitivo de la app e ícono.
- Librería concreta de búsqueda difusa y su umbral.
- Diseño visual dentro del esquema de una sola pantalla.
- Cómo se distribuye (instalador `.msi` y/o `.exe` portable) y la integración continua (CI) en GitHub.
