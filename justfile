build:
    # compila la pagina web
    cargo run -- build -i ./scaffold -f -o ./outputs/build-example
    # -i, --input
    # -f, --force 
    # -o, --output

init:
    # crea el scaffold, usa el directorio ./scaffold 
    cargo run -- init -f -o ./outputs/init-example
    # -f, --force 
    # -o, --output
