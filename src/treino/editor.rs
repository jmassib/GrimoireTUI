//! Uma página de escrever bem simples para o Treino: texto corrido com
//! quebra de linha automática, cursor, setas, apagar. Nada de formatação.

#[derive(Debug, Clone, Default)]
pub struct Editor {
    pub texto: Vec<char>,
    pub cursor: usize,
    /// Primeira linha visual mostrada.
    pub rolagem: usize,
    /// A coluna que as setas ↑↓ tentam manter.
    coluna_alvo: Option<usize>,
    pub alterado: bool,
}

/// Uma linha na tela: o intervalo [inicio, fim) do texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linha {
    pub inicio: usize,
    pub fim: usize,
}

/// Quebra o texto em linhas de no máximo `largura` caracteres, cortando nos
/// espaços quando dá.
pub fn quebrar(texto: &[char], largura: usize) -> Vec<Linha> {
    let w = largura.max(1);
    let mut out = Vec::new();
    let mut ini_logica = 0;
    let n = texto.len();
    loop {
        // fim da linha lógica
        let mut fim = ini_logica;
        while fim < n && texto[fim] != '\n' {
            fim += 1;
        }
        let mut s = ini_logica;
        if s == fim {
            out.push(Linha { inicio: s, fim: s });
        }
        while s < fim {
            if fim - s <= w {
                out.push(Linha { inicio: s, fim });
                break;
            }
            // procura o último espaço dentro da largura
            let limite = s + w;
            let mut corte = None;
            let mut j = limite;
            while j > s {
                if texto[j - 1] == ' ' {
                    corte = Some(j);
                    break;
                }
                j -= 1;
            }
            let c = corte.filter(|c| *c > s).unwrap_or(limite);
            out.push(Linha { inicio: s, fim: c });
            s = c;
        }
        if fim >= n {
            break;
        }
        ini_logica = fim + 1;
    }
    out
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn com_texto(s: &str) -> Self {
        let texto: Vec<char> = s.chars().collect();
        let cursor = texto.len();
        Editor {
            texto,
            cursor,
            ..Default::default()
        }
    }

    pub fn conteudo(&self) -> String {
        self.texto.iter().collect()
    }

    pub fn vazio(&self) -> bool {
        self.texto.iter().all(|c| c.is_whitespace())
    }

    pub fn inserir(&mut self, c: char) {
        if c == '\r' {
            return;
        }
        let c = if c == '\t' { ' ' } else { c };
        self.texto.insert(self.cursor, c);
        self.cursor += 1;
        self.coluna_alvo = None;
        self.alterado = true;
    }

    pub fn colar(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' || !c.is_control() || c == '\t' {
                self.inserir(c);
            }
        }
    }

    pub fn apagar_antes(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.texto.remove(self.cursor);
            self.alterado = true;
        }
        self.coluna_alvo = None;
    }

    pub fn apagar_depois(&mut self) {
        if self.cursor < self.texto.len() {
            self.texto.remove(self.cursor);
            self.alterado = true;
        }
        self.coluna_alvo = None;
    }

    /// Apaga a palavra antes do cursor (Ctrl/Alt-Backspace).
    pub fn apagar_palavra(&mut self) {
        while self.cursor > 0 && self.texto[self.cursor - 1].is_whitespace() {
            self.apagar_antes();
        }
        while self.cursor > 0 && !self.texto[self.cursor - 1].is_whitespace() {
            self.apagar_antes();
        }
    }

    pub fn esquerda(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.coluna_alvo = None;
    }

    pub fn direita(&mut self) {
        self.cursor = (self.cursor + 1).min(self.texto.len());
        self.coluna_alvo = None;
    }

    pub fn palavra_esquerda(&mut self) {
        while self.cursor > 0 && !self.texto[self.cursor - 1].is_alphanumeric() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && self.texto[self.cursor - 1].is_alphanumeric() {
            self.cursor -= 1;
        }
        self.coluna_alvo = None;
    }

    pub fn palavra_direita(&mut self) {
        let n = self.texto.len();
        while self.cursor < n && !self.texto[self.cursor].is_alphanumeric() {
            self.cursor += 1;
        }
        while self.cursor < n && self.texto[self.cursor].is_alphanumeric() {
            self.cursor += 1;
        }
        self.coluna_alvo = None;
    }

    /// Linha e coluna do cursor na tela.
    pub fn posicao(&self, linhas: &[Linha]) -> (usize, usize) {
        for (r, l) in linhas.iter().enumerate() {
            let ultima_da_logica = l.fim >= self.texto.len()
                || self.texto.get(l.fim) == Some(&'\n')
                || linhas.get(r + 1).is_none_or(|p| p.inicio != l.fim);
            if self.cursor >= l.inicio
                && (self.cursor < l.fim || (self.cursor == l.fim && ultima_da_logica))
            {
                return (r, self.cursor - l.inicio);
            }
        }
        let r = linhas.len().saturating_sub(1);
        (r, self.cursor - linhas.get(r).map_or(0, |l| l.inicio))
    }

    fn vertical(&mut self, largura: usize, delta: isize) {
        let linhas = quebrar(&self.texto, largura);
        let (r, c) = self.posicao(&linhas);
        let alvo = *self.coluna_alvo.get_or_insert(c);
        let nr = r as isize + delta;
        if nr < 0 {
            self.cursor = 0;
            return;
        }
        let Some(l) = linhas.get(nr as usize) else {
            self.cursor = self.texto.len();
            return;
        };
        let max = if l.fim > l.inicio
            && linhas
                .get(nr as usize + 1)
                .is_some_and(|p| p.inicio == l.fim)
        {
            // linha quebrada no meio: não deixa o cursor passar para a seguinte
            l.fim - l.inicio - 1
        } else {
            l.fim - l.inicio
        };
        self.cursor = l.inicio + alvo.min(max);
    }

    pub fn acima(&mut self, largura: usize) {
        self.vertical(largura, -1);
    }

    pub fn abaixo(&mut self, largura: usize) {
        self.vertical(largura, 1);
    }

    pub fn inicio_linha(&mut self, largura: usize) {
        let linhas = quebrar(&self.texto, largura);
        let (r, _) = self.posicao(&linhas);
        self.cursor = linhas[r].inicio;
        self.coluna_alvo = None;
    }

    pub fn fim_linha(&mut self, largura: usize) {
        let linhas = quebrar(&self.texto, largura);
        let (r, _) = self.posicao(&linhas);
        let l = linhas[r];
        let quebrada = linhas.get(r + 1).is_some_and(|p| p.inicio == l.fim);
        self.cursor = if quebrada {
            l.fim.saturating_sub(1).max(l.inicio)
        } else {
            l.fim
        };
        self.coluna_alvo = None;
    }

    /// Ajusta a rolagem para o cursor ficar à vista em `altura` linhas.
    pub fn ajustar_rolagem(&mut self, largura: usize, altura: usize) {
        let linhas = quebrar(&self.texto, largura);
        let (r, _) = self.posicao(&linhas);
        let h = altura.max(1);
        if r < self.rolagem {
            self.rolagem = r;
        } else if r >= self.rolagem + h {
            self.rolagem = r + 1 - h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn linhas_de(s: &str, w: usize) -> Vec<String> {
        let t = chars(s);
        quebrar(&t, w)
            .iter()
            .map(|l| t[l.inicio..l.fim].iter().collect())
            .collect()
    }

    #[test]
    fn quebra_nos_espacos() {
        assert_eq!(
            linhas_de("era uma vez um gato", 8),
            vec!["era uma ", "vez um ", "gato"]
        );
        assert_eq!(linhas_de("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(linhas_de("a\n\nb", 10), vec!["a", "", "b"]);
        assert_eq!(linhas_de("", 10), vec![""]);
        assert_eq!(linhas_de("ação\n", 10), vec!["ação", ""]);
    }

    #[test]
    fn digitar_e_apagar() {
        let mut e = Editor::new();
        for c in "olá mundo".chars() {
            e.inserir(c);
        }
        e.apagar_palavra();
        assert_eq!(e.conteudo(), "olá ");
        e.esquerda();
        e.esquerda();
        e.apagar_depois();
        assert_eq!(e.conteudo(), "ol ");
        assert!(e.alterado);
    }

    #[test]
    fn setas_verticais() {
        let mut e = Editor::com_texto("primeira linha\nsegunda");
        e.cursor = 3;
        e.abaixo(40);
        assert_eq!(e.cursor, 15 + 3);
        e.abaixo(40);
        assert_eq!(e.cursor, e.texto.len());
        e.acima(40);
        e.acima(40);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn posicao_no_fim_de_linha_quebrada() {
        let e = Editor::com_texto("era uma vez");
        let l = quebrar(&e.texto, 8);
        assert_eq!(e.posicao(&l), (1, 3));
        let mut e2 = e.clone();
        e2.cursor = 8;
        assert_eq!(e2.posicao(&l), (1, 0));
    }

    #[test]
    fn rolagem_segue_o_cursor() {
        let mut e = Editor::com_texto("1\n2\n3\n4\n5\n6");
        e.ajustar_rolagem(10, 3);
        assert_eq!(e.rolagem, 3);
        e.cursor = 0;
        e.ajustar_rolagem(10, 3);
        assert_eq!(e.rolagem, 0);
    }
}
