# Treino: hábito e exercícios

O modo Treino é para aquecer e criar o hábito antes de ir para o seu texto. Tudo nele é em português do Brasil.

## Como abrir

- No terminal: `grimoire treino` (ou `grimoire treino <pasta>` para apontar a pasta `exercicios/`).
- Dentro do Grimoire: `Ctrl-K` › *Writing* › **Treino: hábito e exercícios…**, ou digite *treino* na busca do menu. O livro é salvo antes.

## Os modos

- **Sessão de hábito** (`1` ou `h`): escolha um texto do `manuscript/` ou comece um novo. Aparecem só o bilhete *[Próximo: …]* e o último parágrafo, para você redigitar e engatar. Timer de 10 minutos (liga e desliga com `F2`); quando acaba, aparece *Pode parar — ou continue*. Com `Esc`, o app pede o bilhete da próxima vez e grava no topo do texto. O bilhete fica só no arquivo: nenhuma exportação (Word, PDF, EPUB, brochura, Markdown) nem a compilação o levam.
- **Exercícios** (`2` ou `e`): escolha um da lista ou *Aleatório* (`a`). A instrução e a meta ficam no topo. *Roleta de ideias* e *Colisão* sorteiam do seu `ideias.md` (`F5` sorteia de novo).
- **Continue a frase** (`3` ou `f`): a abertura de um livro, com autor, obra e tradutor. Escolha um ou *Aleatório*.
- **Dicas** (`4` ou `d`) e **Ir para o meu texto** (`5` ou `g`).

## O limite de 30 minutos

Exercícios e Continue a frase têm no máximo 30 minutos, com contagem regressiva no topo. Aos 25 minutos: *Faltam 5 minutos — prepare-se para ir para o texto*. Aos 30, o treino é salvo e aparece *Tempo de treino acabou — vá para o seu texto*: `Enter` abre o Grimoire no seu livro, `h` abre a sessão de hábito no último texto, `m` volta ao menu e `Esc` deixa terminar a frase. Nada do que foi escrito se perde.

## Teclas ao escrever

`Esc` termina e salva · `Ctrl-S` salva · `Ctrl-G` salva e vai para o seu texto · `Ctrl-Q` salva e sai.

## Os arquivos

Ficam em `exercicios/`, na raiz do repositório (o app procura a partir da pasta atual e do último livro aberto, ou use a variável `GRIMOIRE_EXERCICIOS`): `dicas.md`, `exercicios.md`, `frases.toml`, `ideias.md`. Cada treino vai para `sessoes/AAAA-MM-DD-HHMM-<nome>.md` e cada sessão ganha uma linha em `registro.csv`.
