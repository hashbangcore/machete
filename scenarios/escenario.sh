#!/usr/bin/env bash
#
# Escenarios de la línea de comandos de machete.
#
# Cada escenario corre en su propio directorio temporal, que se borra al
# terminar. Se capturan stdout y stderr por separado a propósito: parte de lo
# que hay que decidir es si un mensaje va a uno u a otro, y no se puede judging
# si todo va mezclado.
#
#   bash escenario.sh <id>

set -uo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MACHETE="$AQUI/../target/debug/machete"
RAIZ="$(mktemp -d "${TMPDIR:-/tmp}/machete-escenario-XXXXXX")"

trap 'rm -rf "$RAIZ"' EXIT
cd "$RAIZ" || exit 1

# --- utilidades -----------------------------------------------------------

# run <orden...>
# Ejecuta la orden y muestra por qué flujo salió cada cosa y con qué código.
run() {
    local fuera_ou fuera_err codigo
    fuera_ou=$(mktemp)
    fuera_err=$(mktemp)

    "$@" >"$fuera_ou" 2>"$fuera_err"
    codigo=$?

    printf '$ %s\n' "$(quitar_raiz "$*")"
    printf '  %s %s\n' "código:" "$codigo"

    local texto flujo linea
    for texto in "$fuera_ou" "$fuera_err"; do
        flujo="stdout"
        [ "$texto" = "$fuera_err" ] && flujo="stderr"
        [ -s "$texto" ] || continue
        while IFS= read -r linea; do
            printf '  %-7s %s\n' "$flujo" "$linea"
        done <"$texto"
    done

    rm -f "$fuera_ou" "$fuera_err"
}

# Imprime la orden sin el directorio temporal delante, que solo añade ruido.
quitar_raiz() {
    printf '%s' "${1//$RAIZ\//}"
}

# espera <descripción> <esperado> <real>
# Anota si lo que pasó coincide con lo que debía pasar.
espera() {
    local descripcion="$1" esperado="$2" real="$3"

    if [ "$esperado" = "$real" ]; then
        printf '  ok       %s\n' "$descripcion"
    else
        printf '  FALLA    %s: esperaba %s, obtuve %s\n' "$descripcion" "$esperado" "$real"
        FALLAS=$((FALLAS + 1))
    fi
}

titulo() {
    printf '\n=== %s\n\n' "$*"
}

existe() { [ -e "$1" ] && printf 'si' || printf 'no'; }

# Lista lo que hay en un directorio, para que se vea de un vistazo.
arbol() {
    find "$1" -type f 2>/dev/null | sort | sed 's|^|  |' || true
    [ -z "$(find "$1" -type f 2>/dev/null)" ] && printf '  (vacío)\n'
    return 0
}

FALLAS=0

# --- el sitio mínimo que usan casi todos los escenarios -------------------

sitio_minimo() {
    mkdir -p templates/partials assets/css
    printf '[data]\ntitle = "Escenario"\n' >machete.toml
    printf '<html><body><content/></body></html>\n' >templates/layout.html
    printf '<html type="layout"/>\n<main><h1>{title}</h1></main>\n' >templates/index.html
    printf 'body{color:green}\n' >assets/css/estilo.css
}

# --- escenarios -----------------------------------------------------------

caso_01() {
    titulo "01  init: el destino no existe"
    run "$MACHETE" init demo
    arbol demo
    espera "machete.toml escrito" "si" "$(existe demo/machete.toml)"
    espera "layout escrito" "si" "$(existe demo/templates/layout.html)"
    espera "assets escritos" "si" "$(existe demo/assets/css/style.css)"
}

caso_02() {
    titulo "02  init: el directorio existe pero está vacío"
    mkdir -p vacio
    run "$MACHETE" init vacio
    arbol vacio
    espera "machete.toml escrito" "si" "$(existe vacio/machete.toml)"
}

caso_03() {
    titulo "03  init: ya hay un fichero y NO se pasa -f"
    mkdir -p demo01
    printf 'contenido mio\n' >demo01/machete.toml

    run "$MACHETE" init demo01
    arbol demo01
    printf '  machete.toml dice: %s\n' "$(head -1 demo01/machete.toml)"

    espera "el fichero del usuario sigue intacto" "si" \
        "$(grep -q 'contenido mio' demo01/machete.toml && printf si || printf no)"
    # Comportamiento por fichero, no todo o nada: el que existe se respeta y los
    # que faltan se escriben igual. Es lo que hace el código hoy.
    espera "los otros cinco ficheros sí se escriben" "5" \
        "$(find demo01 -type f ! -name machete.toml | wc -l | tr -d ' ')"
}

caso_04() {
    titulo "04  init -f: ya hay un fichero y se pasa -f"
    mkdir -p demo02
    printf 'contenido mio\n' >demo02/machete.toml

    run "$MACHETE" init -f demo02
    arbol demo02
    printf '  machete.toml dice: %s\n' "$(head -1 demo02/machete.toml)"

    espera "el scaffold ha sobrescrito" "si" \
        "$(grep -q 'title' demo02/machete.toml && printf si || printf no)"
    espera "se escriben los 6 ficheros del scaffold" "6" \
        "$(find demo02 -type f | wc -l | tr -d ' ')"
}

caso_05() {
    titulo "05  init: destino con carpetas que no existen"
    run "$MACHETE" init uno/dos/tres
    arbol uno
    espera "crea las carpetas intermedias" "si" "$(existe uno/dos/tres/machete.toml)"
}

caso_06() {
    titulo "06  build: el directorio no es un proyecto"
    mkdir -p templates
    printf 'x\n' >templates/index.html
    run "$MACHETE" build
    espera "no escribe nada" "no" "$(existe public)"
}

caso_07() {
    titulo "07  build: hay machete.toml pero no templates/"
    printf '[data]\ntitle = "x"\n' >machete.toml
    run "$MACHETE" build
    espera "no escribe nada" "no" "$(existe public)"
}

caso_08() {
    titulo "08  build: hay templates pero ninguna página"
    mkdir -p templates
    printf '[data]\ntitle = "x"\n' >machete.toml
    run "$MACHETE" build
    espera "no escribe nada" "no" "$(existe public)"
}

caso_09() {
    titulo "09  build: sitio válido"
    sitio_minimo
    run "$MACHETE" build
    arbol public
    espera "escribe index.html" "si" "$(existe public/index.html)"
    espera "copia el asset" "si" "$(existe public/css/estilo.css)"
}

caso_10() {
    titulo "10  build: assets anidados"
    sitio_minimo
    mkdir -p assets/js assets/img
    printf 'console.log(1)\n' >assets/js/app.js
    printf 'svg' >assets/img/logo.svg
    run "$MACHETE" build
    arbol public
    espera "conserva la primera carpeta" "si" "$(existe public/js/app.js)"
    espera "y la segunda" "si" "$(existe public/img/logo.svg)"
}

caso_11() {
    titulo "11  build -f: limpia la salida antes"
    sitio_minimo
    mkdir -p public
    printf 'basura vieja\n' >public/obsoleto.html
    run "$MACHETE" build -f
    arbol public
    espera "el fichero viejo desaparece" "no" "$(existe public/obsoleto.html)"
}

caso_12() {
    titulo "12  build: input:output"
    mkdir -p sitio
    (cd sitio && sitio_minimo)
    run "$MACHETE" build sitio:dist
    arbol dist
    espera "escribe en dist" "si" "$(existe dist/index.html)"
    espera "no toca public" "no" "$(existe public)"
}

caso_13() {
    titulo "13  build: -t y -a"
    mkdir -p plantillas recursos
    printf '[data]\ntitle = "x"\n\n[build]\ntemplates = "noexiste"\nassets = "noexiste"\n' >machete.toml
    printf '<html><body><content/></body></html>\n' >plantillas/layout.html
    printf '<html type="layout"/>\n<main>x</main>\n' >plantillas/index.html
    printf 'h1{color:red}\n' >recursos/estilo.css

    run "$MACHETE" build -t plantillas -a recursos
    arbol public
    espera "lee las plantillas de -t" "si" "$(existe public/index.html)"
    espera "copia los recursos de -a" "si" "$(existe public/estilo.css)"
}

caso_14() {
    titulo "14  build: componente que no existe"
    sitio_minimo
    printf '<html type="layout"/>\n<raro type="component"/>\n' >templates/index.html
    run "$MACHETE" build
    espera "no escribe la página" "no" "$(existe public/index.html)"
}

caso_15() {
    titulo "15  build: variable sin definir"
    sitio_minimo
    printf '<html type="layout"/>\n<main>{noExiste}</main>\n' >templates/index.html
    run "$MACHETE" build
    espera "no escribe la página" "no" "$(existe public/index.html)"
}

caso_16() {
    titulo "16  build: layout por nombre que no existe"
    sitio_minimo
    printf '<html type="layout" name="otro"/>\n<main>x</main>\n' >templates/index.html
    run "$MACHETE" build
    espera "no escribe la página" "no" "$(existe public/index.html)"
}

caso_17() {
    titulo "17  build: script type=data fuera de un componente"
    sitio_minimo
    printf '<html type="layout"/>\n<script type="data">x = "y";</script>\n' >templates/index.html
    run "$MACHETE" build
    espera "no escribe la página" "no" "$(existe public/index.html)"
}

caso_18() {
    titulo "18  sitemap: sin url declarada"
    sitio_minimo
    run "$MACHETE" build
    printf '  sitemap.xml: %s\n' "$(existe public/sitemap.xml)"
    espera "no hay sitemap" "no" "$(existe public/sitemap.xml)"
}

caso_19() {
    titulo "19  sitemap: con url declarada"
    sitio_minimo
    printf '[data]\ntitle = "x"\n\n[build]\nurl = "https://ejemplo.com"\n' >machete.toml
    run "$MACHETE" build
    printf '  sitemap.xml: %s\n' "$(existe public/sitemap.xml)"
    printf '  %s\n' "$(grep -o '<loc>.*</loc>' public/sitemap.xml 2>/dev/null)"
    espera "sí hay sitemap" "si" "$(existe public/sitemap.xml)"
}

caso_20() {
    titulo "20  sitemap: apagado con sitemap = false"
    sitio_minimo
    printf '[data]\ntitle = "x"\n\n[build]\nurl = "https://ejemplo.com"\nsitemap = false\n' >machete.toml
    run "$MACHETE" build
    printf '  sitemap.xml: %s\n' "$(existe public/sitemap.xml)"
    espera "no hay sitemap" "no" "$(existe public/sitemap.xml)"
}

caso_21() {
    titulo "21  flujo completo: init y después build"
    run "$MACHETE" init proyecto
    # Sin tocar el machete.toml que trae el scaffold: el flujo de verdad es este.
    run env -C proyecto "$MACHETE" build
    arbol proyecto/public
    espera "el scaffold construye tal cual" "si" \
        "$(existe proyecto/public/index.html)"
    printf '  el scaffold depende de: '
    grep -o '^[a-z]*' proyecto/machete.toml | tail -n +2 | tr '\n' ' '
    printf '\n'
}

caso_22() {
    titulo "22  completions: cada shell"
    for shell in bash zsh fish powershell elvish; do
        run "$MACHETE" completions "$shell"
    done
}

caso_23() {
    titulo "23  completions: shell que no existe"
    run "$MACHETE" completions tcsh
}

caso_24() {
    titulo "24  --help y --version"
    run "$MACHETE" --help
    run "$MACHETE" build --help
    run "$MACHETE" --version
}

caso_25() {
    titulo "25  sin argumentos: no se eligió comando"
    run "$MACHETE"
}

# --- despacho -------------------------------------------------------------

main() {
    local id="${1:-}"
    case "$id" in
        01) caso_01 ;;
        02) caso_02 ;;
        03) caso_03 ;;
        04) caso_04 ;;
        05) caso_05 ;;
        06) caso_06 ;;
        07) caso_07 ;;
        08) caso_08 ;;
        09) caso_09 ;;
        10) caso_10 ;;
        11) caso_11 ;;
        12) caso_12 ;;
        13) caso_13 ;;
        14) caso_14 ;;
        15) caso_15 ;;
        16) caso_16 ;;
        17) caso_17 ;;
        18) caso_18 ;;
        19) caso_19 ;;
        20) caso_20 ;;
        21) caso_21 ;;
        22) caso_22 ;;
        23) caso_23 ;;
        24) caso_24 ;;
        25) caso_25 ;;
        *)
            printf 'Escenario desconocido: %s. Los disponibles: 01 a 25\n' "${id:-<ninguno>}" >&2
            return 2
            ;;
    esac

    return "$FALLAS"
}

main "$@"