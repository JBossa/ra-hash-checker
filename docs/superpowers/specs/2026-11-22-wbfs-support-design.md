# Soporte de `.wbfs` — diseño corto

**Por qué:** los usuarios de Wii suelen tener sus juegos en `.wbfs` (USB Loader GX, Wii Backup Manager) y la app
solo acepta `.iso`, así que hoy tienen que convertir cada juego antes de comprobarlo.

## Hallazgo clave
`rc_hash_wii` (`vendor/rcheevos/src/rhash/hash_disc.c:1219-1345`) solo lee: la cabecera (0x80 bytes), el código de
región (0x4E000), la tabla de particiones (0x40000) y, por cada partición que no sea de actualización, su TMD y sus
primeros 1024 clústeres cifrados (~32 MB). Un `.wbfs` guarda los sectores del disco sin modificar más una tabla de
bloques; solo descarta bloques sin uso (típicamente la partición de actualización, que el hash ignora). Si se sirve
a rcheevos un lector que traduzca posiciones del ISO a posiciones del `.wbfs`, el hash es el del `.iso`.

## Objetivos
- `.wbfs` (y sus partes `.wbf1`, `.wbf2`… de FAT32) se comprueba como Wii, sin convertir ni escribir a disco.
- Mismo MD5 que el `.iso` equivalente.

## No objetivos
- `.rvz`, `.gcz`, `.wia`, `.cso`: comprimidos/re-cifrados; siguen con el mensaje de convertir a `.iso`.
- `.wbfs` con varios juegos (particiones WBFS de disco entero): solo se lee el primer juego.

## Enfoques
1. **Elegido:** lector virtual (callbacks `rc_hash_filereader_t` del iterador) en el crate `rcheevos-hash`.
2. Convertir a un `.iso` temporal: ~4,7 GB de escritura por juego. Descartado.
3. Herramienta externa (`wit`): dependencia que el usuario tendría que instalar. Descartado.

## Formato (libwbfs, igual que el lector WBFS de Dolphin)
- Cabecera en 0: `"WBFS"`, `n_hd_sec` (u32 BE), `hd_sec_sz_s` (u8), `wbfs_sec_sz_s` (u8).
- Info del disco 0 en `1 << hd_sec_sz_s`: copia de la cabecera del disco (0x100) y tabla `wlba` (u16 BE) de
  `(143432 * 2) >> (wbfs_sec_sz_s - 15)` entradas.
- Posición `p` del ISO → bloque `p >> wbfs_sec_sz_s`; `wlba == 0` son ceros; si no,
  `(wlba << wbfs_sec_sz_s) + (p & máscara)` dentro de la concatenación de `x.wbfs`, `x.wbf1`, `x.wbf2`…

## Riesgo
Si un bloque que lee el hash estuviera vacío en el `.wbfs`, saldrían ceros y el MD5 no coincidiría. Es la zona con
datos del juego, así que no se espera; se valida con un `.wbfs` real que coincida con un juego registrado.
