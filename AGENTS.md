# Machete — Especificación de diseño

## 1. Propósito

Machete es una herramienta escrita en Rust para modularizar y componer HTML y generar páginas HTML finales.

Su responsabilidad se limita a la composición de documentos mediante componentes, layouts y variables. No pretende ser un motor de plantillas de propósito general.

Queda fuera de su alcance:

- Bucles y condicionales.
- Procesamiento de datos.
- Ejecución de JavaScript.
- Sustituir herramientas como esbuild o Deno.

## 2. Configuración

El proyecto utiliza un fichero `machete.toml` para definir las rutas y las variables globales.

La presencia de ese fichero es lo que identifica el directorio como un proyecto de Machete. Si falta, `machete build` informa de ello en lugar de seguir adelante con las variables vacías: de otro modo, una variable olvidada se reportaría como si estuviera mal definida y la causa real quedaría oculta.

Estructura propuesta:

```
.
├── machete.toml
├── templates/
│   ├── index.html
│   ├── about.html
│   ├── layout.html
│   ├── layouts/
│   │   ├── article.html
│   │   └── gallery.html
│   └── partials/
│       ├── header.html
│       ├── navigation.html
│       └── footer.html
├── static/
└── public/
```

- `templates/`: páginas de entrada.
- `templates/partials/`: definiciones de componentes.
- `templates/layout.html`: layout predeterminado opcional.
- `templates/layouts/`: layouts adicionales opcionales.
- `static/`: recursos estáticos.
- `public/`: directorio de salida.

Las rutas deben ser configurables. Esta estructura es una propuesta predeterminada, no una obligación para todos los proyectos.

## 3. Componentes

### 3.1. Identificación

El atributo `type="component"` identifica un elemento que Machete debe procesar como componente.

```
<header type="component"></header>
```

El nombre de la etiqueta identifica el componente. Ese nombre corresponde al fichero homónimo en el directorio `partials/`:

```
<header type="component"></header>   →   partials/header.html
<aside   type="component"></aside>   →   partials/aside.html
<figure  type="component"></figure>  →   partials/figure.html
```

Cada fichero contiene un solo componente. Machete no valida los nombres: si el autor elige un nombre que no es una etiqueta HTML válida, es una decisión suya y un linter puede avisar. Machete solo informa cuando el componente invocado no existe.

### 3.2. Definición de un componente

Las definiciones de componentes se almacenan exclusivamente en el directorio configurado para `partials`.

Por ejemplo, `templates/partials/header.html`:

```
<header>
  <h1>{title}</h1>
  <p>{description}</p>
  <content/>
</header>
```

Esta definición contiene:

- HTML que formará parte del resultado.
- Variables que Machete sustituirá por sus valores.
- Un marcador `<content/>` que indica dónde insertar el contenido proporcionado por quien invoca el componente.

Las etiquetas y los atributos de la definición se conservan en el HTML generado, salvo las construcciones que Machete deba procesar o retirar.

### 3.3. Invocación de un componente

Una etiqueta con `type="component"` fuera del directorio de definiciones representa una invocación.

```
<header type="component">
  <script type="data">
    const title = "Mi sitio";
    const description = "Página personal";
  </script>

  <nav>
    <a href="/">Inicio</a>
    <a href="/about.html">Acerca de</a>
  </nav>
</header>
```

Machete busca la definición correspondiente en `partials/`, resuelve las variables y compone el resultado.

Si no existe la definición del componente solicitado, debe producir un error.

Una invocación sin contenido puede escribirse también auto-cerrada:

```
<header type="component" title="Mi sitio"/>
```

### 3.4. Formas de proporcionar variables

Un componente puede recibir variables de dos maneras:

Mediante atributos:

Los atributos de la invocación proporcionan valores al componente. El nombre del atributo es el nombre de la variable. Todos los atributos que no sean `type` se consumen como variables y no aparecen en el HTML final.

Mediante el componente especial `script`:

`<script type="data">` permite definir variables para la invocación. En este contexto, su contenido declara valores para Machete; no significa que Machete deba ejecutar JavaScript arbitrario.

```
<aside type="component">
  <script type="data">
    const title = "Sobre este ejercicio";
    const link = "https://example.com";
  </script>
</aside>
```

El bloque no es JavaScript. Machete solo aprovecha el resaltado de sintaxis y los literales que el editor ya reconoce. Las palabras clave `const`, `let` y `var` son opcionales, y `clave = valor` es tan válido como `const clave = valor`. Todos los valores son cadenas de texto: no hay inferencia de tipos, y `true` es el texto `"true"`.

El bloque solo puede aparecer dentro de un componente. Un `<script type="data">` suelto es un error.

Los nombres y valores declarados se utilizan para resolver las variables de la definición del componente.

### 3.5. Inserción de contenido

`<content/>` marca el lugar donde se inserta el contenido HTML proporcionado al invocar un componente. El marcador desaparece de la salida: en su lugar queda el contenido recibido.

```
<!-- partials/figure.html -->
<figure class="note">
  <content/>
</figure>
```

```
<figure type="component">
  <p>Este texto entra por el marcador.</p>
</figure>
```

Si la definición no contiene `<content/>`, no existe un punto de inserción para ese contenido. Para evitar perder contenido silenciosamente, se genera un error cuando se proporcione contenido y la definición no tenga el marcador.

Una definición con `<content/>` puede invocarse sin contenido. El marcador se elimina igualmente y queda un elemento vacío.

Si una definición declara `<content/>` más de una vez, se genera un error: el destino sería ambiguo.

## 4. Variables

### 4.1. Sintaxis

Las llaves identifican las variables:

```
<h1>{title}</h1>
<meta name="description" content="{description}">
```

Machete sustituye `{title}` y `{description}` por sus valores correspondientes.

No se requieren etiquetas especiales para marcar las referencias a variables dentro del HTML.

### 4.2. Scope global

Las variables globales se definen en `machete.toml` y están disponibles para las páginas, los layouts y los componentes.

Las variables globales viven en `[data]`:

```toml
[data]
title = "machete"
```

Un layout puede tener las suyas con `[data.<layout>]`, que heredan de `[data]` y pisan lo que repitan:

```toml
[data]
title = "machete"
lang = "es"

[data.about]
title = "Acerca de"
```

Con ese fichero, la página `<html type="layout" name="about"/>` recibe `title` = `Acerca de` y `lang` = `es`. Un layout sin sección propia recibe solo `[data]`.

`[data]` admite un solo nivel. Anidar más, como `[data.blog.post]`, es un error.

Machete lee únicamente `[data]`, `[data.<layout>]` y `[build]`. Cualquier otra tabla o clave del fichero es del usuario y no se inyecta, para que pueda guardar lo que necesite:

```toml
author = "hash"

[ci]
run = "cargo test"

[data]
title = "machete"
```

Las rutas viven en `[build]` y tienen valores por defecto, así que no necesitan declararse:

```toml
[build]
public = "public"
templates = "templates"
```

Con ese fichero, el sitio lee las plantillas de `templates/` y escribe en `public/`.

### 4.3. Scope local

Las variables locales se definen para cada componente mediante sus atributos o el componente especial `script`.

Su alcance corresponde a la invocación del componente; no modifican los valores globales.

### 4.4. Precedencia

Si existe una variable local y otra global con el mismo nombre, prevalece la local.

Ejemplo conceptual:

- Variable global `title`: `Mi sitio`.
- Variable local `title`: `Mi artículo`.
- Dentro del componente se utiliza `Mi artículo`.
- El valor global sigue siendo `Mi sitio` para el resto del documento.

## 5. Layouts

### 5.1. Layout predeterminado

Una página declara que necesita un layout mediante `<html type="layout"/>`. La directiva no se renderiza: se extrae de la página y el resto del contenido es lo que se entrega al layout.

Por ejemplo, una página de entrada puede contener únicamente:

```
<html type="layout"/>

<main>
  <h1>Mi sitio</h1>
  <p>Contenido principal.</p>
</main>
```

Y `layout.html` contiene la estructura general del documento:

```
<!DOCTYPE html>
<html lang="es">
<head type="component" title="{title}"/>

<body>
  <main>
    <content/>
  </main>
</body>
</html>
```

El contenido de la página se inserta en `<content/>`.

La página se inserta en el layout **antes** de expandir los componentes, porque el marcador puede estar dentro de una invocación:

```
<body>
  <main type="component">
    <content/>
  </main>
</body>
```

Si se expandiera antes, el marcador desaparecería dentro de la definición y no habría dónde insertar la página.

### 5.2. Layouts adicionales

Una página solicita otro layout añadiendo `name`:

```
<html type="layout" name="article"/>
```

Ese layout se busca en el directorio `layouts/`:

```
templates/
├── layout.html
└── layouts/
    ├── article.html
    └── gallery.html
```

Si se solicita un layout que no existe, Machete debe producir un error.

## 6. Procesamiento del HTML

El procesamiento debe respetar estas reglas:

1. Leer la configuración y resolver las rutas.
2. Cargar las variables globales.
3. Identificar el layout correspondiente a cada página.
4. Resolver las invocaciones de componentes contra sus definiciones en `partials/`.
5. Resolver las variables locales y globales, dando prioridad a las locales.
6. Insertar el contenido en los marcadores `<content/>`.
7. Generar los documentos HTML finales en el directorio de salida.

El orden exacto de algunas operaciones internas queda sujeto a la implementación, siempre que se respeten las reglas de composición y alcance.

### 6.1. La salida es HTML estándar

Las plantillas son la entrada y contienen sintaxis de Machete. Los ficheros generados en `public/` son la salida y **no contienen ninguna construcción de Machete**:

- Ni `<content/>`, ni `type="component"`, ni `type="layout"`, ni `<script type="data">`.

El resultado debe poder servirse tal cual, sin transformaciones posteriores.

## 7. Errores que deben detectarse

Machete debería informar, al menos, de los siguientes errores:

- Componente invocado sin definición correspondiente.
- Layout solicitado que no existe.
- Contenido proporcionado a un componente sin marcador `<content/>`.
- `<content/>` declarado más de una vez en la misma definición.
- Variable duplicada en una invocación, recibida por atributo y por `script`.
- `<script type="data">` fuera de un componente.
- Variables sin resolver.
- Atributos o marcadores mal formados.

Una referencia `{variable}` sin valor es un error: el build no compila. No se conserva literalmente.

## 8. Límites y responsabilidades

Machete compone HTML. No ejecuta JavaScript ni pretende sustituir herramientas especializadas.

Por tanto, el código JavaScript ordinario debe conservarse para que otras herramientas lo procesen. El `<script type="data">` es una construcción especial de Machete para declarar variables, no un mecanismo de ejecución de código. Su gramática es `clave = valor`; solo se aprovechan el resaltado de sintaxis y los literales que el editor reconoce.

### 8.1. Autocierre

`<content/>`, `type="component"` y `type="layout"` se autocierran, igual que los elementos void de HTML. El resto de las etiquetas sigue el estándar: `<div/>` no cierra nada y es un error.

### 8.2. Nombres de componente

El nombre de un componente es el nombre del fichero que lo contiene, sin extensión. Machete no valida esos nombres ni los de la etiqueta que lo invoca. Si el autor elige un nombre que HTML no considera válido, es una decisión suya y un linter puede avisar. Machete solo informa cuando el componente invocado no existe.

## 9. Decisiones pendientes

- El comportamiento ante variables inexistentes dentro del contenido insertado en un componente: si ese contenido ve las variables locales del componente o solo las globales.

Estas cuestiones deben cerrarse antes de implementar el parser, para evitar que la implementación termine imponiendo decisiones de diseño que todavía no se han tomado.
