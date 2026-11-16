# Requisitos para compilar o Grimoire (este fork)

Do que você precisa para compilar e rodar o Grimoire no macOS e no Linux. No repositório `writings`, o `REQUISITOS.md` da raiz também cobre o conversor de textos e traz o script `instalar.sh`, que faz tudo de uma vez.

## Rust 1.88 ou mais novo

A versão mínima é a **1.88**: o ratatui 0.30 e o `time` 0.3.55 pedem essa versão. Ela também está em `rust-version` no `Cargo.toml`, então o cargo avisa claramente se o Rust for velho demais.

- Ainda não tem Rust? Instale pelo [rustup](https://rustup.rs):
  `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Já tem? Atualize com `rustup update`. Confira com `rustc --version`.

O Rust do Homebrew (`brew install rust`) também serve, desde que seja 1.88 ou mais novo. O rustup é mais simples de manter atualizado.

## Pacotes do sistema

| Sistema | O que instalar | Para quê |
|---|---|---|
| macOS | Ferramentas de linha de comando do Xcode: `xcode-select --install` | compilador e linker; nada mais |
| Debian / Ubuntu | `sudo apt install build-essential pkg-config libasound2-dev` | compilador; ALSA para a música |
| Fedora | `sudo dnf install gcc pkgconf-pkg-config alsa-lib-devel` | idem |
| Arch | `sudo pacman -S base-devel pkgconf alsa-lib` | idem |

No Linux, o ALSA (`libasound2-dev`) só é necessário para a **música**, uma função ligada por padrão. Sem ele, compile com `--no-default-features`: tudo funciona, inclusive o Treino, menos a música.

### Opcional: PDF

Para exportar em PDF (o manuscrito e a brochura), o Grimoire usa o **LibreOffice**, que é gratuito. Os outros formatos (Word, EPUB, Markdown) funcionam sem ele.

- macOS: `brew install --cask libreoffice`, ou baixe em libreoffice.org
- Debian / Ubuntu: `sudo apt install libreoffice`
- Fedora: `sudo dnf install libreoffice`
- Arch: `sudo pacman -S libreoffice-fresh`

## Compilar e instalar

```sh
cargo build --release --locked            # gera target/release/grimoire
cargo install --path . --locked           # instala o comando `grimoire` em ~/.cargo/bin
```

Sem a música (Linux sem ALSA):

```sh
cargo build --release --locked --no-default-features
cargo install --path . --locked --no-default-features
```

O `~/.cargo/bin` precisa estar no `PATH`. O rustup já cuida disso; se não, abra um terminal novo ou rode `source ~/.cargo/env`.

## Testes

```sh
cargo test --workspace                          # tudo
cargo test --workspace --no-default-features    # sem a música
```

## Rodar

```sh
grimoire                  # abre o livro da pasta atual (ou o último aberto)
grimoire treino           # o modo Treino (hábito, exercícios, Continue a frase)
grimoire export --docx    # exporta sem abrir a tela
```
