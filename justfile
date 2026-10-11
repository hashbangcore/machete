install:
    cargo install --path .

commit hint="":
    gareth repo commit {{ hint }} | git commit --edit -F -
