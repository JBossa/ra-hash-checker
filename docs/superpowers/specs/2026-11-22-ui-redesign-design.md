# Rediseño de la UI — diseño corto

**Por qué:** la v0.1.0 funciona, pero en modo oscuro las tarjetas salen blancas con texto claro, la zona
de soltar desperdicia espacio, el historial de muchos archivos es difícil de leer y una coincidencia solo
muestra un nombre y un enlace.

## Objetivos
- Estilo visual cercano al de RetroAchievements: fondos gris carbón, acentos dorados/ámbar, tarjetas sobrias.
- Zona de soltar grande y legible cuando no hay resultados; barra compacta cuando los hay.
- Historial ordenado: resumen por estado, filtros (todos / registrados / no registrados / otros), botón limpiar, más recientes arriba.
- Coincidencia = ficha del juego: portada, título, consola, editora/desarrolladora/año, logros, hash del usuario,
  todos los hashes compatibles del juego con el que coincidió resaltado (y sus etiquetas).
- Ajustes como panel con secciones (clave con mostrar/ocultar y enlace para obtenerla; caché con última actualización).
- Logo original nuevo (SVG) reutilizado para los iconos de la app.

## No objetivos / anti-objetivos
- No copiar el logo ni marcas de RetroAchievements (solo el estilo general).
- No cambiar cómo se calculan los hashes ni la lógica de coincidencia.
- No añadir frameworks de frontend: sigue siendo TypeScript + DOM.

## Enfoques considerados
1. **Elegido:** ampliar `CheckResult` con `gameHashes` (ya se descargan al coincidir) y un comando nuevo
   `game_info(gameId)` que la UI pide de forma perezosa para la portada y los metadatos (`API_GetGame.php`),
   cacheado en disco en `info/{id}.json`. Las filas aparecen igual de rápido y la ficha se completa después.
2. Meter la info del juego dentro de `check`: más simple en la UI pero cada coincidencia esperaría una petición más.
3. Pedir la API directamente desde el webview: expondría la clave al JS. Descartado.

## Decisiones
- Imágenes desde `https://media.retroachievements.org` (CSP es nula; el webview las cachea).
- Sin conexión o sin datos: la ficha muestra un hueco con el icono de consola en vez de portada.
- Ventana por defecto 960×720, mínimo 640×520.
- Tema oscuro por defecto (como RA); en modo claro del sistema se usa una variante clara con los mismos acentos.

## Para quien lo construya
Detalles de espaciado, tipografía y animaciones; textos en `i18n.ts` (es/en).
