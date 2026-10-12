# Propuestas: mensajes de la línea de comandos

Notas de trabajo sobre lo que se vio al ejecutar la CLI en distintos escenarios y
sobre cómo unificar esos mensajes. No es una implementación: son opciones, con lo
que cuesta cada una, para decidir antes de escribir nada.

Los escenarios viven en `escenario.sh` y se ejecutan con `just test-<número>` o
todos con `just todos`.

---

## 1. Lo que se ejecutó

25 escenarios repartidos así:

| Grupo | Escenarios | Qué cubren |
|---|---|---|
| `init` | 01 a 05 | destino nuevo, vacío, con contenido, con `-f`, anidado |
| `build` de base | 06 a 08 | sin config, sin plantillas, sin páginas |
| `build` normal | 09 a 13 | válido, assets anidados, `-f`, `input:output`, `-t`/`-a` |
| `build` con fallo | 14 a 17 | componente, variable, layout y bloque de datos |
| `sitemap` | 18 a 20 | sin `url`, con `url`, apagado |
| Varios | 21 a 25 | init+build, completions, ayuda, sin comandos |

---

## 2. Lo que se encontró

### 2.1 Todo lo que funciona no dice absolutamente nada

Escenarios 01, 02, 05, 09, 10, 11, 12, 13, 19, 21 y 22 terminan con código 0 y
**cero líneas de salida**. Ni «escrito», ni «6 ficheros», ni dónde.

Un caso concreto: `machete build` de un sitio válido termina mudo. Si hay
código de salida, se puede preguntar; si se está mirando la pantalla, no hay
nada que leer.

### 2.2 `init` sin `-f` escribe parte del scaffold sin decir nada

Este es el escenario 03, el que motivó todo:

```
$ machete init demo01          # demo01/machete.toml ya existe
  código: 0
  demo01/machete.toml                    <- intacto, sigue siendo el del usuario
  demo01/templates/layout.html            <- escritos
  demo01/templates/index.html             <- escritos
  demo01/templates/partials/head.html     <- escritos
  demo01/templates/partials/header.html   <- escritos
  demo01/assets/css/style.css             <- escritos
  (nada)
```

La comprobación es **por fichero**, no todo o nada: respeta el que existe y
escribe los que faltan. En `src/build.rs:119` hay un `continue` que no dice nada.

Lo de menos a más:

- No avisa de que el `machete.toml` del usuario se ha respetado.
- No avisa de que los otros cinco ficheros sí se han escrito encima de un
  directorio que ya tenía contenido.

El resultado es un directorio a medio camino, sin ninguna pista de en qué estado
queda.

### 2.3 Los saltos del sitemap tampoco dicen nada

Escenarios 18 y 20. En ambos se termina con código 0 y sin una línea, pero por
motivos distintos:

- 18: no hay `url`, así que no hay sitemap. Es el comportamiento previsto.
- 20: `sitemap = false`. Es una decisión del usuario.

El primero se distingue del segundo porque el segundo es explícito. En los dos
casos hay una pregunta razonable sin respuesta: «¿escribiste el sitemap?».

### 2.4 Un fallo deja el directorio de salida a medias

Escenario 08: `machete build` con plantillas pero sin páginas falla con «no pages
to build», y sin embargo deja `public/` creado y vacío.

La causa está en el orden de `src/build.rs`: `create_dir_all` (línea 42) ocurre
antes de comprobar que haya páginas (línea 41 del `if`). Un build que falla
deja un directorio que no estaba.

### 2.5 Hay dos códigos de salida para el mismo tipo de problema

| Situación | Código | Quién lo imprime |
|---|---|---|
| Error propio de machete | **1** | `main.rs` |
| Valor inválido de clap (`completions tcsh`) | **2** | clap |
| Sin subcomando (`machete` a secas) | **2** | clap |
| `--help` | 0 | clap |
| Éxito | 0 | machete |

No es un bug grave, pero un script que traite cualquier error igual acabará
tratando de forma distinta un error de machete y un error de línea de comandos.

Además, `machete` sin subcomando **imprime la ayuda a `stderr`**. La ayuda
general suele ir a `stdout` cuando se pide explícitamente (`--help`, que sí va a
`stdout` con código 0).

### 2.6 Los errores están bien redactados

Esto no hay que cambiarlo. Salen por `stderr`, con prefijo `error: `, dicen qué
falta y qué hacer:

```
error: . has no machete.toml, so it is not a machete project. Run `machete init`
       here to create one, or point the input at a directory that has one
error: ./templates/index.html: variable 'noExiste' is not defined. Set it in
       machete.toml, or pass it to the component as an attribute
error: ./templates/index.html: no definition for component 'raro'
```

Los tres incluyen el fichero, que es lo que hace falta para arreglarlo.

### 2.7 El scaffold depende de más variables de las que parece

Lo destapó el escenario 21. Si alguien deja `[data]` con solo `title`, que es lo
natural, el scaffold **no construye**: el componente `head` pide `{description}`.

No es un fallo del scaffold, es que su `machete.toml` trae cinco variables
(`title`, `description`, `repo`, `docs`, `web`) y no es obvio que haya que
mantenerlas todas. Merece una nota en el propio scaffold.

---

## 3. Dónde debería dispararse cada mensaje

Punto de partida, con el fichero y el sitio exacto:

| Momento | Dónde | Tipo | Texto de ejemplo |
|---|---|---|---|
| `init` respeta un fichero existente | `src/build.rs:119`, en el `continue` | aviso | `machete.toml ya existe, se deja como está` |
| `init` termina | `src/cli.rs:163`, tras `write_scaffold` | éxito | `escrito en demo01/` |
| `build` termina | `src/cli.rs:161`, tras `build` | éxito | `4 páginas y 2 recursos en public/` |
| `build` no escribe sitemap por falta de `url` | `src/build.rs:84`, el `return Ok(())` de `write_sitemap` | aviso | `sin sitemap: falta url en [build]` |
| `build` no escribe sitemap porque se apagó | `src/build.rs:88` | **nada** | Es una decisión explícita, avisar sería ruido |
| Error en cualquier punto | `src/main.rs`, el `Err(error)` | error | lo que ya hay |
| `completions` genera el script | `src/cli.rs` | nada | Su salida **es** el script, no puede llevar mensajes encima |

Los dos casos del sitemap se distinguen a propósito: uno es una omisión y el otro
es una orden. Si los dos avisan, quien apagó el sitemap ve un aviso que no le
aporta nada.

---

## 4. Propuestas de mecanismo

### A. Una función libre por nivel

```rust
pub fn success(mensaje: &str) { … }
pub fn warning(mensaje: &str) { … }
pub fn error(mensaje: &str)   { … }
```

Lo más rápido de escribir. Cada una decide flujo, prefijo y color.

- A favor: cero ceremonia, se lee igual en cualquier sitio.
- En contra: **no se puede probar**. Para comprobar que un aviso sale habría que
  mirar los descriptores del proceso. Y con `-q` haría falta un estado global que
  las tres lean.

### B. Un informe que se pasa por todas partes

```rust
pub struct Reporte { silencioso: bool, color: bool }
impl Reporte {
    pub fn exito(&self, mensaje: &str)   { … }
    pub fn aviso(&self, mensaje: &str)   { … }
    pub fn error(&self, mensaje: &str)   { … }
}
```

Un valor que se construye en `cli.rs` y se pasa a `build` y `write_scaffold`.

- A favor: sin estado global, `-q` es un campo, y las pruebas pueden construirse
  un `Reporte` de mentira que recoja lo escrito.
- En contra: hay que cambiar las firmas de `build` y `write_scaffold`, que ahora
  reciben `&Config` y `bool` y son públicas.

### C. Un trait con dos implementaciones

```rust
pub trait Informe {
    fn exito(&self, mensaje: &str);
    fn aviso(&self, mensaje: &str);
    fn error(&self, mensaje: &str);
}

pub struct InformeNormal;   // colorea y escribe
pub struct InformeMudo;     // no hace nada, para -q
```

Se elige con un `Box<dyn Informe>` en tiempo de ejecución.

- A favor: `-q` es una implementación más, no un `if` repartido. Añadir un nivel
  («info», «depuración») es añadir un método.
- En contra: más ceremonia que un struct. Un `Box<dyn>` para tres métodos que son
  tres `println` es mucha máquina para poca cosa.

### Recomendación

**B**, por una razón concreta: el proyecto ya tiene 170 pruebas, y el único
mecanismo de mensajes que se puede probar sin lanzar un proceso es uno que se
inyecte. `A` es la que se acaba escribiendo sin darse cuenta, y dentro de seis
mesos no hay forma de comprobar que el aviso de `init` sigue saliendo.

Si más adelante hacen falta varios formatos de salida —un `--json` para CI, un
log estructurado— la `B` se convierte en `C` sin refactorizar nada.

---

## 5. La flag `-q` / `--quiet`

Lo que hace `-q` no está claro todavía, y hay dos lecturas:

| Lectura | Qué se calla | Cuándo se usa |
|---|---|---|
| **Silencioso** | Todo menos los errores | En scripts y CI, donde la salida se analiza |
| **Poco verboso** | Los detalles, no los resúmenes | Cuando quieres «12 páginas» y no «aquí está cada una» |

La primera es la estándar en las herramientas de línea de comandos y la que
encaja con que exista un único mensaje por operación. En esa, el comportamiento
quedaría:

```
machete build                 machete build -q
─────────────────────         ─────────────────────
✓ 4 páginas en public/        (nada)
```

y los errores **siguen saliendo** con `-q`, porque un fallo silencioso en un
script es peor que un poco de ruido:

```
$ machete build -q
error: ./templates/index.html: variable 'noExiste' is not defined
```

Si además se quiere poder cortar los errores, haría falta una segunda flag
(`--silent`), y no parece justificado.

## 6. Los colores

Lo natural: `error` en rojo, `aviso` en amarillo, `exito` en verde.

Lo que hay que decidir:

- **Cuándo se colorea.** Hay que comprobar que la salida es una terminal. Al
  escribir a un fichero o a un pipe no hay color. La comprobación estándar es
  mirar si `stderr` es una `tty`.
- **`NO_COLOR`.** Es el convenio de `no-color.org` y hoy lo respeta poca gente.
  Son tres líneas y evita tener un fichero con marcas de escape.
- **`CLICOLOR_FORCE`.** Para quien quiere color en un pipe. Opcional.

En la práctica esto significa una comprobación al arrancar, no una en cada
mensaje.

---

## 7. Los códigos de salida

Hoy conviven el 1 de machete y el 2 de clap. Propuesta:

| Código | Cuándo |
|---|---|
| 0 | Todo fue bien, incluidos los avisos |
| 1 | Error de machete |
| 2 | Error de línea de comandos (lo emite clap) |

El 1 y el 2 ya están casi igual por casualidad, así que basta con **documentar
lo que ya hay** en vez de cambiarlo. Lo que sí convendría arreglar:

- `machete` sin subcomando imprime la ayuda a `stderr` y sale con 2, mientras que
  `--help` la imprime a `stdout` y sale con 0. Es la convención opuesta de la
  habitual: la ayuda que no se pidió a propósito es un error, y a `stderr`;
  la que se pidió con `--help` es un resultado, y a `stdout`. **Lo que hay hoy
  es lo correcto** y solo falta dejarlo escrito.

---

## 8. Orden sugerido

1. **`-q`/`--quiet` y el mecanismo de mensajes**, empezando por el aviso de
   `init`, que es el que más caro sale cuando falta: escribir a medias sin
   enterarse.
2. **Mensajes de éxito** de `init` y `build`, con el recuento de lo escrito.
3. **Aviso del sitemap omitido** por falta de `url`.
4. **Colores**, con la comprobación de terminal y `NO_COLOR`.
5. **Lo del directorio a medias** del 2.4: mover `create_dir_all` después de
   comprobar que hay páginas, o borrar el directorio si el build falla.
6. **La nota del scaffold** del 2.7, que no necesita código: basta con una línea
   en el `machete.toml` del scaffold diciendo que las cinco variables hacen falta.

Los puntos 5 y 6 no dependen del sistema de mensajes y se pueden hacer en
cualquier momento.