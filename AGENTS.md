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

El proyecto utiliza un fichero `config.toml` para definir las rutas y las variables globales.

Estructura propuesta:

```
.
├── config.toml
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

El nombre de la etiqueta identifica el componente. En este caso, `header` corresponde a una definición ubicada en el directorio `partials/`.

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
  <script type="component">
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

### 3.4. Formas de proporcionar variables

Un componente puede recibir variables de dos maneras:

Mediante atributos:

Los atributos de la invocación proporcionan valores al componente. El mecanismo exacto para asociar cada atributo con una variable debe mantener el nombre de la variable como referencia.

Mediante el componente especial `script`:

`<script type="component">` permite definir variables para la invocación. En este contexto, su contenido declara valores para Machete; no significa que Machete deba ejecutar JavaScript arbitrario.

Los nombres y valores declarados se utilizan para resolver las variables de la definición del componente.

### 3.5. Inserción de contenido

`<content/>` marca el lugar donde se inserta el contenido HTML proporcionado al invocar un componente.

Si la definición no contiene `<content/>`, no existe un punto de inserción para ese contenido. Para evitar perder contenido silenciosamente, la opción recomendada es generar un error cuando se proporcione contenido y la definición no tenga el marcador.

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

Las variables globales se definen en `config.toml` y están disponibles para las páginas, los layouts y los componentes.

El formato exacto para declararlas en el fichero de configuración todavía debe definirse.

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

Si existe `layout.html` y la página de entrada no define una etiqueta `<html>`, Machete aplica el layout automáticamente.

Por ejemplo, una página de entrada puede contener únicamente:

```
<h1>Mi sitio</h1>
<p>Contenido principal.</p>
```

Y `layout.html` puede contener la estructura general del documento:

```
<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="UTF-8">
  <title>{title}</title>
</head>
<body>
  <header type="component"></header>
  <main>
    <content/>
  </main>
</body>
</html>
```

El contenido de la página se inserta en `<content/>`.

Si la página ya define su propia etiqueta `<html>`, no se aplica automáticamente el layout predeterminado.

### 5.2. Layouts adicionales

Cuando se necesitan varios layouts, se utiliza el directorio `layouts/`.

```
templates/
├── layout.html
└── layouts/
    ├── article.html
    └── gallery.html
```

La selección explícita de un layout se realizará mediante una declaración en la página. La sintaxis definitiva de esa declaración aún está pendiente.

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

## 7. Errores que deben detectarse

Machete debería informar, al menos, de los siguientes errores:

- Componente invocado sin definición correspondiente.
- Layout solicitado que no existe.
- Contenido proporcionado a un componente sin marcador `<content/>`, si se adopta la regla recomendada.
- Variables sin resolver, si se decide que las referencias inexistentes son errores en lugar de conservarse literalmente.

El último comportamiento todavía debe decidirse.

## 8. Límites y responsabilidades

Machete compone HTML. No ejecuta JavaScript ni pretende sustituir herramientas especializadas.

Por tanto, el código JavaScript ordinario debe conservarse para que otras herramientas lo procesen. El `<script type="component">` es una construcción especial de Machete para declarar variables, no un mecanismo de ejecución de código.

## 9. Decisiones pendientes

La base conceptual está definida, pero faltan algunas decisiones para convertirla en una especificación implementable:

- La estructura exacta de `config.toml`, incluidas las variables globales.
- La sintaxis definitiva para seleccionar un layout distinto del predeterminado.
- La gramática exacta de las declaraciones dentro de `<script type="component">`.
- La correspondencia entre atributos de invocación y variables.
- El comportamiento ante variables inexistentes.
- La política frente a atributos o marcadores mal formados.

Estas cuestiones deben cerrarse antes de implementar el parser, para evitar que la implementación termine imponiendo decisiones de diseño que todavía no se han tomado.
