//! Os arquivos do modo Treino, todos texto simples e editáveis à mão:
//!
//! ```text
//! exercicios/
//!   dicas.md         as dicas de hábito          (## Título / texto / Meta:)
//!   exercicios.md    os exercícios               (## Título / texto / Meta: / Ideias:)
//!   frases.toml      aberturas para Continue a frase ([[frase]])
//!   ideias.md        suas ideias, uma por linha
//!   registro.csv     uma linha por sessão (criado pelo app)
//!   sessoes/         cada treino salvo como AAAA-MM-DD-HHMM-<nome>.md
//!   textos/          textos novos começados numa sessão de hábito
//! ```

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DICAS_PADRAO: &str = include_str!("../../assets/exercicios/dicas.md");
pub const EXERCICIOS_PADRAO: &str = include_str!("../../assets/exercicios/exercicios.md");
pub const FRASES_PADRAO: &str = include_str!("../../assets/exercicios/frases.toml");
pub const IDEIAS_PADRAO: &str = include_str!("../../assets/exercicios/ideias.md");

pub const CABECALHO_REGISTRO: &str = "data,hora,modo,atividade,minutos,palavras,arquivo";

/// Uma dica ou um exercício.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub titulo: String,
    pub texto: String,
    /// None quando é "sem meta definida" ou não há linha Meta.
    pub meta: Option<String>,
    /// Quantas ideias sortear do ideias.md (linha "Ideias: N").
    pub ideias: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Frase {
    pub autor: String,
    pub obra: String,
    #[serde(default)]
    pub ano: Option<i32>,
    #[serde(default)]
    pub tradutor: Option<String>,
    pub texto: String,
    #[serde(default)]
    pub fonte: Option<String>,
}

impl Frase {
    /// "Clarice Lispector, A hora da estrela (1977)" e o tradutor, se houver.
    pub fn credito(&self) -> String {
        let mut s = format!("{}, {}", self.autor, self.obra);
        if let Some(a) = self.ano {
            s.push_str(&format!(" ({a})"));
        }
        if let Some(t) = self.tradutor.as_deref().filter(|t| !t.trim().is_empty()) {
            s.push_str(&format!(" · tradução de {t}"));
        }
        s
    }
}

#[derive(Deserialize)]
struct ArquivoFrases {
    #[serde(default)]
    frase: Vec<Frase>,
}

/// Tira os comentários `<!-- … -->` (mesmo os de várias linhas).
fn sem_comentarios(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find("<!--") {
        out.push_str(&resto[..i]);
        match resto[i..].find("-->") {
            Some(j) => resto = &resto[i + j + 3..],
            None => {
                resto = "";
                break;
            }
        }
    }
    out.push_str(resto);
    out
}

/// "1. Roleta de ideias" → "Roleta de ideias".
fn sem_numero(t: &str) -> &str {
    let t = t.trim();
    let digitos = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digitos > 0 {
        let r = &t[digitos..];
        if let Some(r) = r.strip_prefix('.').or_else(|| r.strip_prefix(')')) {
            return r.trim();
        }
    }
    t
}

fn meta_de(v: &str) -> Option<String> {
    let v = v.trim();
    let limpo = v.trim_end_matches('.').trim();
    if limpo.is_empty() || limpo.eq_ignore_ascii_case("sem meta definida") {
        None
    } else {
        Some(limpo.to_string())
    }
}

/// Lê dicas.md ou exercicios.md.
pub fn ler_itens(texto: &str) -> Vec<Item> {
    let texto = sem_comentarios(texto);
    let mut itens: Vec<Item> = Vec::new();
    let mut atual: Option<Item> = None;
    let mut linhas: Vec<String> = Vec::new();
    let fechar = |item: Option<Item>, linhas: &mut Vec<String>, itens: &mut Vec<Item>| {
        if let Some(mut it) = item {
            it.texto = linhas
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            itens.push(it);
        }
        linhas.clear();
    };
    for l in texto.lines() {
        let t = l.trim();
        if let Some(titulo) = t.strip_prefix("## ") {
            fechar(atual.take(), &mut linhas, &mut itens);
            atual = Some(Item {
                titulo: sem_numero(titulo).to_string(),
                ..Default::default()
            });
            continue;
        }
        let Some(it) = atual.as_mut() else { continue };
        if t.starts_with('#') {
            continue;
        }
        if let Some(v) = t.strip_prefix("Meta:") {
            it.meta = meta_de(v);
        } else if let Some(v) = t.strip_prefix("Ideias:") {
            it.ideias = v.trim().parse().unwrap_or(0).min(10);
        } else if !t.is_empty() {
            linhas.push(t.to_string());
        }
    }
    fechar(atual.take(), &mut linhas, &mut itens);
    itens
}

pub fn ler_frases(texto: &str) -> Result<Vec<Frase>> {
    let a: ArquivoFrases = toml::from_str(texto).context("frases.toml com erro de formato")?;
    Ok(a.frase
        .into_iter()
        .filter(|f| !f.texto.trim().is_empty())
        .collect())
}

/// Uma ideia por linha; ignora títulos, comentários, linhas vazias e
/// marcadores de lista.
pub fn ler_ideias(texto: &str) -> Vec<String> {
    sem_comentarios(texto)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let l = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .unwrap_or(l);
            sem_numero(l).to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// Onde ficam os arquivos e o que se achou neles.
#[derive(Debug, Clone)]
pub struct Pasta {
    pub dir: PathBuf,
    /// Por que foi esta pasta (para dizer ao usuário quando é a reserva).
    pub aviso: Option<String>,
}

impl Pasta {
    pub fn dicas(&self) -> PathBuf {
        self.dir.join("dicas.md")
    }
    pub fn exercicios(&self) -> PathBuf {
        self.dir.join("exercicios.md")
    }
    pub fn frases(&self) -> PathBuf {
        self.dir.join("frases.toml")
    }
    pub fn ideias(&self) -> PathBuf {
        self.dir.join("ideias.md")
    }
    pub fn registro(&self) -> PathBuf {
        self.dir.join("registro.csv")
    }
    pub fn sessoes(&self) -> PathBuf {
        self.dir.join("sessoes")
    }
    pub fn textos(&self) -> PathBuf {
        self.dir.join("textos")
    }
    /// A pasta do livro/repositório que contém exercicios/, se houver.
    pub fn raiz(&self) -> Option<PathBuf> {
        self.dir.parent().map(Path::to_path_buf)
    }

    /// Cria o que falta (nunca sobrescreve).
    pub fn completar(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("criando {}", self.dir.display()))?;
        for (p, c) in [
            (self.dicas(), DICAS_PADRAO),
            (self.exercicios(), EXERCICIOS_PADRAO),
            (self.frases(), FRASES_PADRAO),
            (self.ideias(), IDEIAS_PADRAO),
        ] {
            if !p.exists() {
                std::fs::write(&p, c).with_context(|| format!("criando {}", p.display()))?;
            }
        }
        Ok(())
    }
}

/// Procura `exercicios/` em `dir` e nas pastas acima dela.
pub fn subir_procurando(dir: &Path) -> Option<PathBuf> {
    let mut d = Some(dir);
    while let Some(p) = d {
        if p.file_name().is_some_and(|n| n == "exercicios") && p.join("exercicios.md").is_file() {
            return Some(p.to_path_buf());
        }
        let c = p.join("exercicios");
        if c.join("exercicios.md").is_file() || c.join("frases.toml").is_file() {
            return Some(c);
        }
        d = p.parent();
    }
    None
}

/// A ordem de procura: a variável GRIMOIRE_EXERCICIOS, a pasta pedida,
/// a pasta atual (e as de cima), o último livro aberto (e as de cima).
/// Se nada for achado, usa a pasta de reserva e a cria com os arquivos padrão.
pub fn localizar(pedida: Option<&Path>, ultimo_livro: Option<&Path>, reserva: &Path) -> Pasta {
    if let Ok(v) = std::env::var("GRIMOIRE_EXERCICIOS")
        && !v.trim().is_empty()
    {
        return Pasta {
            dir: PathBuf::from(v),
            aviso: None,
        };
    }
    if let Some(p) = pedida {
        if let Some(d) = subir_procurando(p) {
            return Pasta {
                dir: d,
                aviso: None,
            };
        }
        let d = if p.file_name().is_some_and(|n| n == "exercicios") {
            p.to_path_buf()
        } else {
            p.join("exercicios")
        };
        return Pasta {
            dir: d,
            aviso: Some("pasta exercicios/ criada com os arquivos padrão".into()),
        };
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Some(d) = subir_procurando(&cwd)
    {
        return Pasta {
            dir: d,
            aviso: None,
        };
    }
    if let Some(l) = ultimo_livro
        && let Some(d) = subir_procurando(l)
    {
        return Pasta {
            dir: d,
            aviso: None,
        };
    }
    Pasta {
        dir: reserva.to_path_buf(),
        aviso: Some(format!(
            "não achei a pasta exercicios/ do seu repositório; usando {}",
            reserva.display()
        )),
    }
}

// ── O bilhete "[Próximo: …]" e o último parágrafo ─────────────────────

/// Separa o frontmatter YAML (`---` … `---`) do resto, sem mexer nele.
pub fn separar_frontmatter(s: &str) -> (&str, &str) {
    if let Some(r) = s
        .strip_prefix("---\n")
        .or_else(|| s.strip_prefix("---\r\n"))
    {
        let mut pos = 0;
        for linha in r.split_inclusive('\n') {
            if linha.trim_end() == "---" {
                let fim = s.len() - r.len() + pos + linha.len();
                return (&s[..fim], &s[fim..]);
            }
            pos += linha.len();
        }
    }
    ("", s)
}

fn bilhete_da_linha(l: &str) -> Option<String> {
    let t = l.trim();
    let dentro = t.strip_prefix('[')?.strip_suffix(']')?;
    let (rotulo, resto) = dentro.split_once(':')?;
    let r = rotulo.trim().to_lowercase();
    (r == "próximo" || r == "proximo").then(|| resto.trim().to_string())
}

/// O texto dividido em frontmatter, bilhete do topo e corpo.
#[derive(Debug, Clone, PartialEq)]
pub struct Texto {
    pub frontmatter: String,
    pub bilhete: Option<String>,
    pub corpo: String,
}

pub fn abrir_texto(s: &str) -> Texto {
    let (fm, resto) = separar_frontmatter(s);
    let resto_t = resto.trim_start_matches(['\n', '\r']);
    let primeira = resto_t.lines().next().unwrap_or("");
    let (bilhete, corpo) = match bilhete_da_linha(primeira) {
        Some(b) => (
            Some(b),
            resto_t[primeira.len()..].trim_start_matches(['\n', '\r']),
        ),
        None => (None, resto_t),
    };
    Texto {
        frontmatter: fm.to_string(),
        bilhete,
        corpo: corpo.to_string(),
    }
}

pub fn montar_texto(t: &Texto) -> String {
    let mut s = t.frontmatter.clone();
    if !s.is_empty() {
        s.push('\n');
    }
    if let Some(b) = t.bilhete.as_deref().filter(|b| !b.trim().is_empty()) {
        s.push_str(&format!("[Próximo: {}]\n\n", b.trim()));
    }
    s.push_str(t.corpo.trim_end());
    s.push('\n');
    s
}

fn paragrafos(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut atual: Vec<&str> = Vec::new();
    for l in s.lines() {
        if l.trim().is_empty() {
            if !atual.is_empty() {
                v.push(atual.join("\n"));
                atual.clear();
            }
        } else {
            atual.push(l);
        }
    }
    if !atual.is_empty() {
        v.push(atual.join("\n"));
    }
    v
}

pub fn ultimo_paragrafo(corpo: &str) -> Option<String> {
    paragrafos(corpo).pop()
}

fn palavras_norm(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

fn lcs(a: &[String], b: &[String]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    for x in a {
        let mut cur = vec![0usize; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Se `novo` é uma redigitação de `antigo` (80% das palavras na ordem, e
/// pelo menos do mesmo tamanho).
pub fn e_redigitacao(antigo: &str, novo: &str) -> bool {
    let a = palavras_norm(antigo);
    let b = palavras_norm(novo);
    if a.is_empty() || b.len() * 10 < a.len() * 8 {
        return false;
    }
    lcs(&a, &b) * 10 >= a.len() * 8
}

/// No editor, cada Enter é um parágrafo; no arquivo, parágrafos ficam
/// separados por uma linha em branco, como no resto do Grimoire.
pub fn para_markdown(digitado: &str) -> String {
    digitado
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string()
}

/// O corpo novo de um texto depois de uma sessão de hábito: o que já existia
/// mais o que foi escrito. Se o primeiro parágrafo escrito redigita o último
/// parágrafo do texto, ele toma o lugar desse parágrafo (em vez de repetir).
pub fn juntar(base: &str, digitado: &str) -> String {
    let novo = para_markdown(digitado);
    let base = base.trim_end();
    if novo.is_empty() {
        return base.to_string();
    }
    if base.trim().is_empty() {
        return novo;
    }
    let primeiro = novo.split("\n\n").next().unwrap_or("");
    if let Some(ult) = ultimo_paragrafo(base)
        && e_redigitacao(&ult, primeiro)
        && let Some(pos) = base.rfind(ult.as_str())
    {
        let antes = base[..pos].trim_end();
        return if antes.is_empty() {
            novo
        } else {
            format!("{antes}\n\n{novo}")
        };
    }
    format!("{base}\n\n{novo}")
}

pub fn contar_palavras(s: &str) -> usize {
    s.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

/// "Roleta de ideias" → "roleta-de-ideias" (sem acentos, para nomes de arquivo).
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            c => c.to_ascii_lowercase(),
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let s = out.trim_end_matches('-');
    let s: String = s.chars().take(50).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "texto".into() } else { s }
}

/// `sessoes/AAAA-MM-DD-HHMM-<slug>.md`, sem sobrescrever um que já exista.
pub fn caminho_sessao(
    sessoes: &Path,
    quando: chrono::DateTime<chrono::Local>,
    nome: &str,
) -> PathBuf {
    let base = format!("{}-{}", quando.format("%Y-%m-%d-%H%M"), slug(nome));
    let mut p = sessoes.join(format!("{base}.md"));
    let mut n = 2;
    while p.exists() {
        p = sessoes.join(format!("{base}-{n}.md"));
        n += 1;
    }
    p
}

fn csv(campo: &str) -> String {
    if campo.contains([',', '"', '\n']) {
        format!("\"{}\"", campo.replace('"', "\"\""))
    } else {
        campo.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Registro<'a> {
    pub quando: chrono::DateTime<chrono::Local>,
    /// "habito", "exercicio" ou "frase".
    pub modo: &'a str,
    pub atividade: &'a str,
    pub minutos: u64,
    pub palavras: usize,
    pub arquivo: &'a str,
}

/// Acrescenta uma linha ao registro.csv (criando-o com o cabeçalho).
pub fn registrar(arquivo: &Path, r: &Registro) -> Result<()> {
    use std::io::Write;
    let novo = !arquivo.exists()
        || std::fs::metadata(arquivo)
            .map(|m| m.len() == 0)
            .unwrap_or(true);
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(arquivo)
        .with_context(|| format!("abrindo {}", arquivo.display()))?;
    if novo {
        writeln!(f, "{CABECALHO_REGISTRO}")?;
    }
    writeln!(
        f,
        "{},{},{},{},{},{},{}",
        r.quando.format("%Y-%m-%d"),
        r.quando.format("%H:%M"),
        csv(r.modo),
        csv(r.atividade),
        r.minutos,
        r.palavras,
        csv(r.arquivo)
    )?;
    Ok(())
}

/// O último texto trabalhado numa sessão de hábito, segundo o registro.
pub fn ultimo_texto(registro: &Path) -> Option<String> {
    let s = std::fs::read_to_string(registro).ok()?;
    s.lines()
        .rev()
        .filter_map(|l| {
            let campos = dividir_csv(l);
            (campos.len() >= 7 && campos[2] == "habito").then(|| campos[6].clone())
        })
        .next()
}

fn dividir_csv(l: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    let mut aspas = false;
    let mut chars = l.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if aspas && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => aspas = !aspas,
            ',' if !aspas => v.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    v.push(cur);
    v
}

/// Grava escrevendo antes num arquivo ao lado, para nunca deixar o texto
/// pela metade.
pub fn gravar(p: &Path, conteudo: &str) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("criando {}", d.display()))?;
    }
    let tmp = p.with_extension("md.salvando");
    std::fs::write(&tmp, conteudo).with_context(|| format!("gravando {}", tmp.display()))?;
    std::fs::rename(&tmp, p).with_context(|| format!("gravando {}", p.display()))?;
    Ok(())
}

/// Os textos que uma sessão de hábito pode continuar: os .md de
/// manuscript/ e de exercicios/textos/, os mais recentes primeiro.
pub fn textos_para_continuar(pasta: &Pasta) -> Vec<PathBuf> {
    fn andar(d: &Path, out: &mut Vec<PathBuf>, prof: usize) {
        if prof > 8 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let nome = e.file_name().to_string_lossy().to_string();
            if nome.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                andar(&p, out, prof + 1);
            } else if p.extension().is_some_and(|x| x == "md")
                && !nome.eq_ignore_ascii_case("README.md")
            {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    if let Some(r) = pasta.raiz() {
        andar(&r.join("manuscript"), &mut v, 0);
    }
    andar(&pasta.textos(), &mut v, 0);
    v.sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    v.truncate(300);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exercicios_padrao_sao_lidos() {
        let v = ler_itens(EXERCICIOS_PADRAO);
        assert_eq!(v.len(), 7);
        assert_eq!(v[0].titulo, "Roleta de ideias");
        assert_eq!(v[0].meta.as_deref(), Some("10 minutos"));
        assert_eq!(v[0].ideias, 1);
        assert_eq!(v[5].titulo, "Colisão");
        assert_eq!(v[5].ideias, 2);
        assert_eq!(v[6].meta.as_deref(), Some("100 palavras, depois uma cena"));
        assert!(
            v[1].texto
                .starts_with("Pegue uma ideia e escreva cinco frases")
        );
        assert!(v.iter().all(|i| !i.texto.contains("Meta:")));
    }

    #[test]
    fn dicas_padrao_e_sem_meta() {
        let v = ler_itens(DICAS_PADRAO);
        assert_eq!(v.len(), 7);
        assert_eq!(v[0].titulo, "Pare no meio da frase");
        assert_eq!(v[0].meta, None, "sem meta definida não mostra meta");
        assert_eq!(v[5].meta, None);
        assert_eq!(v[4].meta.as_deref(), Some("10 minutos"));
        assert!(
            v[1].texto
                .contains("[Próximo: ela encontra a carta, ele mente sobre ela]")
        );
    }

    #[test]
    fn formato_flexivel() {
        let s = "intro ignorada\n<!-- ## Falso\n -->\n## Um\nlinha a\nlinha b\nMeta: Sem meta definida\n\n## 2) Dois\nMeta: 3 páginas.\nIdeias: x\n";
        let v = ler_itens(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].texto, "linha a linha b");
        assert_eq!(v[0].meta, None);
        assert_eq!(v[1].titulo, "Dois");
        assert_eq!(v[1].meta.as_deref(), Some("3 páginas"));
        assert_eq!(v[1].ideias, 0);
    }

    #[test]
    fn frases_padrao_sao_validas() {
        let v = ler_frases(FRASES_PADRAO).unwrap();
        assert!(v.len() >= 40, "{} frases", v.len());
        for f in &v {
            assert!(!f.autor.trim().is_empty() && !f.obra.trim().is_empty());
            assert!(f.texto.chars().count() <= 220, "longa demais: {}", f.texto);
            assert!(f.fonte.as_deref().is_some_and(|u| u.starts_with("http")));
        }
        let kafka = v.iter().find(|f| f.obra == "A metamorfose").unwrap();
        assert_eq!(kafka.tradutor.as_deref(), Some("Modesto Carone"));
        assert!(kafka.credito().contains("tradução de Modesto Carone"));
    }

    #[test]
    fn frases_com_erro_dizem_o_que_houve() {
        assert!(ler_frases("[[frase]]\nautor = 1").is_err());
        assert!(ler_frases("").unwrap().is_empty());
    }

    #[test]
    fn ideias() {
        assert!(ler_ideias(IDEIAS_PADRAO).is_empty());
        let v = ler_ideias(
            "# Ideias\n\n- um homem que coleciona chaves\n2. A carta\n  \n<!-- x -->\nnoite de São João\n",
        );
        assert_eq!(
            v,
            vec![
                "um homem que coleciona chaves",
                "A carta",
                "noite de São João"
            ]
        );
    }

    #[test]
    fn bilhete_e_frontmatter() {
        let s = "---\ntitle: X\n---\n\n[Próximo: ela encontra a carta]\n\nPrimeiro.\n\nÚltimo parágrafo aqui.\n";
        let t = abrir_texto(s);
        assert_eq!(t.frontmatter, "---\ntitle: X\n---\n");
        assert_eq!(t.bilhete.as_deref(), Some("ela encontra a carta"));
        assert_eq!(
            ultimo_paragrafo(&t.corpo).as_deref(),
            Some("Último parágrafo aqui.")
        );
        let mut t2 = t.clone();
        t2.bilhete = Some("ele mente".into());
        let out = montar_texto(&t2);
        assert_eq!(
            out,
            "---\ntitle: X\n---\n\n[Próximo: ele mente]\n\nPrimeiro.\n\nÚltimo parágrafo aqui.\n"
        );
        assert_eq!(abrir_texto(&out).bilhete.as_deref(), Some("ele mente"));
        let sem = abrir_texto("Só texto.\n");
        assert_eq!(sem.bilhete, None);
        assert_eq!(montar_texto(&sem), "Só texto.\n");
        assert_eq!(abrir_texto("[proximo: a]\nb").bilhete.as_deref(), Some("a"));
    }

    #[test]
    fn redigitar_substitui_em_vez_de_repetir() {
        let base = "Um começo.\n\nEla abriu a porta devagar e viu a carta no chão.";
        let r = juntar(
            base,
            "Ela abriu a porta devagar, e viu a carta no chão.\nE então ela leu.",
        );
        assert_eq!(
            r,
            "Um começo.\n\nEla abriu a porta devagar, e viu a carta no chão.\n\nE então ela leu."
        );
        let r = juntar(base, "Outra coisa completamente diferente.");
        assert_eq!(r, format!("{base}\n\nOutra coisa completamente diferente."));
        assert_eq!(juntar(base, "   \n"), base);
        assert_eq!(juntar("", "a\nb"), "a\n\nb");
        assert!(!e_redigitacao("um dois três quatro cinco", "um dois"));
    }

    #[test]
    fn nomes_de_arquivo() {
        assert_eq!(slug("Roleta de ideias"), "roleta-de-ideias");
        assert_eq!(
            slug("A primeira frase, de cinco jeitos"),
            "a-primeira-frase-de-cinco-jeitos"
        );
        assert_eq!(slug("Colisão!"), "colisao");
        assert_eq!(slug("???"), "texto");
        let d = std::env::temp_dir().join(format!("treino-slug-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        use chrono::TimeZone;
        let q = chrono::Local
            .with_ymd_and_hms(2026, 11, 15, 18, 40, 0)
            .unwrap();
        let p = caminho_sessao(&d, q, "Colisão");
        assert_eq!(p.file_name().unwrap(), "2026-11-15-1840-colisao.md");
        std::fs::write(&p, "x").unwrap();
        assert_eq!(
            caminho_sessao(&d, q, "Colisão").file_name().unwrap(),
            "2026-11-15-1840-colisao-2.md"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn registro_csv() {
        let d = std::env::temp_dir().join(format!("treino-reg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("registro.csv");
        use chrono::TimeZone;
        let q = chrono::Local
            .with_ymd_and_hms(2026, 11, 15, 9, 5, 0)
            .unwrap();
        let r = Registro {
            quando: q,
            modo: "habito",
            atividade: "Sessão de hábito",
            minutos: 12,
            palavras: 340,
            arquivo: "manuscript/a, b.md",
        };
        registrar(&f, &r).unwrap();
        registrar(
            &f,
            &Registro {
                modo: "exercicio",
                atividade: "Colisão",
                arquivo: "exercicios/sessoes/x.md",
                ..r.clone()
            },
        )
        .unwrap();
        let s = std::fs::read_to_string(&f).unwrap();
        let linhas: Vec<_> = s.lines().collect();
        assert_eq!(linhas[0], CABECALHO_REGISTRO);
        assert_eq!(
            linhas[1],
            "2026-11-15,09:05,habito,Sessão de hábito,12,340,\"manuscript/a, b.md\""
        );
        assert_eq!(linhas.len(), 3);
        assert_eq!(ultimo_texto(&f).as_deref(), Some("manuscript/a, b.md"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn localiza_subindo_e_usa_reserva() {
        let d = std::env::temp_dir().join(format!("treino-loc-{}", std::process::id()));
        let ex = d.join("exercicios");
        std::fs::create_dir_all(d.join("manuscript/livro/manuscript")).unwrap();
        std::fs::create_dir_all(&ex).unwrap();
        std::fs::write(ex.join("exercicios.md"), EXERCICIOS_PADRAO).unwrap();
        assert_eq!(
            subir_procurando(&d.join("manuscript/livro")),
            Some(ex.clone())
        );
        let p = localizar(
            Some(&d.join("manuscript/livro")),
            None,
            Path::new("/nao/existe"),
        );
        assert_eq!(p.dir, ex);
        assert!(p.aviso.is_none());
        let reserva = d.join("reserva/exercicios");
        let p = localizar(None, Some(Path::new("/")), &reserva);
        if std::env::var("GRIMOIRE_EXERCICIOS").is_err()
            && subir_procurando(&std::env::current_dir().unwrap()).is_none()
        {
            assert_eq!(p.dir, reserva);
            assert!(p.aviso.is_some());
            p.completar().unwrap();
            assert!(reserva.join("frases.toml").is_file());
            std::fs::write(reserva.join("ideias.md"), "minha ideia\n").unwrap();
            p.completar().unwrap();
            assert_eq!(
                std::fs::read_to_string(reserva.join("ideias.md")).unwrap(),
                "minha ideia\n",
                "nunca sobrescreve"
            );
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn palavras() {
        assert_eq!(contar_palavras("Era uma vez — um gato."), 5);
        assert_eq!(contar_palavras(""), 0);
    }
}
