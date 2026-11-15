//! Modo Treino: sessão de hábito, exercícios e Continue a frase.
//!
//! Abre com `grimoire treino [pasta]`, ou pelo menu do Grimoire
//! (Ctrl-K › Writing › "Treino: hábito e exercícios…", ou pela busca do
//! palette: "treino"). Todo o texto que aparece para o usuário é em
//! português do Brasil. Os dados ficam em `exercicios/` (ver `dados`).

pub mod dados;
pub mod editor;
pub mod tempo;

use anyhow::Result;
use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dados::{Frase, Item, Pasta, Texto};
use editor::{Editor, quebrar};
use tempo::{Fase, Sorteio};

/// O que fazer quando o Treino fecha.
#[derive(Debug, Clone, PartialEq)]
pub enum Saida {
    Sair,
    /// Abrir o Grimoire no livro (None: o de sempre).
    AbrirLivro(Option<PathBuf>),
}

pub struct Config {
    /// `grimoire treino <pasta>`.
    pub pedida: Option<PathBuf>,
    /// O último livro aberto no Grimoire.
    pub ultimo_livro: Option<PathBuf>,
    /// Onde criar exercicios/ se não houver nenhuma.
    pub reserva: PathBuf,
}

const LARGURA_MAX: u16 = 76;
const AUTOSALVAR: Duration = Duration::from_secs(60);

enum Tela {
    Menu {
        sel: usize,
    },
    Dicas {
        rolagem: u16,
    },
    EscolherTexto {
        sel: usize,
        itens: Vec<PathBuf>,
        ultimo: Option<PathBuf>,
    },
    NomeTextoNovo {
        entrada: String,
    },
    ListaExercicios {
        sel: usize,
    },
    ListaFrases {
        sel: usize,
        ordem: Vec<usize>,
    },
    Escrevendo(Box<Sessao>),
    Bilhete {
        sessao: Box<Sessao>,
        entrada: String,
    },
}

enum Tipo {
    Habito {
        base: Texto,
        ultimo: Option<String>,
        timer: bool,
        timer_avisado: bool,
    },
    Exercicio {
        item: Item,
        aleatorio: bool,
        sorteadas: Vec<String>,
        sem_ideias: bool,
    },
    Frase {
        frase: Frase,
        aleatoria: bool,
    },
}

struct Sessao {
    tipo: Tipo,
    editor: Editor,
    inicio: Instant,
    quando: DateTime<Local>,
    /// O texto (hábito) ou o arquivo em sessoes/ (treino).
    arquivo: PathBuf,
    aviso_25_dado: bool,
    modal_esgotado: bool,
    esgotado_visto: bool,
    ultimo_salvo: Instant,
    registrado: bool,
}

impl Sessao {
    fn nova(tipo: Tipo, arquivo: PathBuf) -> Self {
        // Só para testar os avisos sem esperar: GRIMOIRE_TREINO_ADIANTAR=25
        // começa o relógio já com 25 minutos.
        let adiantar = std::env::var("GRIMOIRE_TREINO_ADIANTAR")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|m| Duration::from_secs(m * 60))
            .unwrap_or_default();
        let inicio = Instant::now()
            .checked_sub(adiantar)
            .unwrap_or_else(Instant::now);
        Sessao {
            tipo,
            editor: Editor::new(),
            inicio,
            quando: Local::now(),
            arquivo,
            aviso_25_dado: false,
            modal_esgotado: false,
            esgotado_visto: false,
            ultimo_salvo: Instant::now(),
            registrado: false,
        }
    }

    fn e_treino(&self) -> bool {
        !matches!(self.tipo, Tipo::Habito { .. })
    }

    fn titulo(&self) -> String {
        match &self.tipo {
            Tipo::Habito { .. } => "Sessão de hábito".into(),
            Tipo::Exercicio { item, .. } => item.titulo.clone(),
            Tipo::Frase { frase, .. } => {
                format!("Continue a frase: {}, {}", frase.autor, frase.obra)
            }
        }
    }

    fn modo(&self) -> &'static str {
        match self.tipo {
            Tipo::Habito { .. } => "habito",
            Tipo::Exercicio { .. } => "exercicio",
            Tipo::Frase { .. } => "frase",
        }
    }

    fn palavras(&self) -> usize {
        dados::contar_palavras(&self.editor.conteudo())
    }

    /// O conteúdo do arquivo de um treino.
    fn arquivo_treino(&self) -> String {
        let mut s = String::new();
        match &self.tipo {
            Tipo::Exercicio {
                item, sorteadas, ..
            } => {
                s.push_str(&format!("# Exercício: {}\n\n", item.titulo));
                s.push_str(&format!(
                    "- Data: {}\n",
                    self.quando.format("%d/%m/%Y %H:%M")
                ));
                if let Some(m) = &item.meta {
                    s.push_str(&format!("- Meta: {m}\n"));
                }
                for (i, ideia) in sorteadas.iter().enumerate() {
                    if sorteadas.len() > 1 {
                        s.push_str(&format!("- Ideia {}: {ideia}\n", i + 1));
                    } else {
                        s.push_str(&format!("- Ideia sorteada: {ideia}\n"));
                    }
                }
                s.push_str(&format!("\n> {}\n", item.texto));
            }
            Tipo::Frase { frase, .. } => {
                s.push_str(&format!(
                    "# Continue a frase: {}, {}\n\n",
                    frase.autor, frase.obra
                ));
                s.push_str(&format!(
                    "- Data: {}\n\n",
                    self.quando.format("%d/%m/%Y %H:%M")
                ));
                s.push_str(&format!("> {}\n>\n> — {}\n", frase.texto, frase.credito()));
            }
            Tipo::Habito { .. } => {}
        }
        s.push_str("\n---\n\n");
        s.push_str(self.editor.conteudo().trim_end());
        s.push('\n');
        s
    }

    /// Grava sem perguntar nada. Num treino vazio não cria arquivo.
    fn salvar(&mut self, bilhete: Option<&str>) -> Result<bool> {
        self.ultimo_salvo = Instant::now();
        match &self.tipo {
            Tipo::Habito { base, .. } => {
                if self.editor.vazio() && bilhete.is_none() {
                    return Ok(false);
                }
                let mut t = base.clone();
                t.corpo = dados::juntar(&base.corpo, &self.editor.conteudo());
                if let Some(b) = bilhete.filter(|b| !b.trim().is_empty()) {
                    t.bilhete = Some(b.trim().to_string());
                }
                dados::gravar(&self.arquivo, &dados::montar_texto(&t))?;
            }
            _ => {
                if self.editor.vazio() {
                    return Ok(false);
                }
                dados::gravar(&self.arquivo, &self.arquivo_treino())?;
            }
        }
        self.editor.alterado = false;
        Ok(true)
    }
}

struct Treino {
    pasta: Pasta,
    dicas: Vec<Item>,
    exercicios: Vec<Item>,
    frases: Vec<Frase>,
    sorteio: Sorteio,
    tela: Tela,
    msg: String,
    ultimo_livro: Option<PathBuf>,
    saida: Option<Saida>,
}

fn rel(raiz: Option<&Path>, p: &Path) -> String {
    raiz.and_then(|r| p.strip_prefix(r).ok())
        .unwrap_or(p)
        .display()
        .to_string()
}

fn sino() {
    use std::io::Write;
    let mut o = std::io::stdout();
    let _ = o.write_all(b"\x07");
    let _ = o.flush();
}

impl Treino {
    fn new(cfg: &Config) -> Self {
        let pasta = dados::localizar(
            cfg.pedida.as_deref(),
            cfg.ultimo_livro.as_deref(),
            &cfg.reserva,
        );
        let mut msg = String::new();
        if let Err(e) = pasta.completar() {
            msg = format!("não consegui criar os arquivos: {e:#}");
        } else if let Some(a) = &pasta.aviso {
            msg = a.clone();
        }
        let mut t = Treino {
            pasta,
            dicas: vec![],
            exercicios: vec![],
            frases: vec![],
            sorteio: Sorteio::do_relogio(),
            tela: Tela::Menu { sel: 0 },
            msg,
            ultimo_livro: cfg.ultimo_livro.clone(),
            saida: None,
        };
        t.recarregar();
        t
    }

    /// Lê de novo os arquivos (para valer o que foi editado à mão).
    fn recarregar(&mut self) {
        let ler = |p: PathBuf, padrao: &str| {
            std::fs::read_to_string(p).unwrap_or_else(|_| padrao.to_string())
        };
        self.dicas = dados::ler_itens(&ler(self.pasta.dicas(), dados::DICAS_PADRAO));
        self.exercicios = dados::ler_itens(&ler(self.pasta.exercicios(), dados::EXERCICIOS_PADRAO));
        match dados::ler_frases(&ler(self.pasta.frases(), dados::FRASES_PADRAO)) {
            Ok(f) => self.frases = f,
            Err(e) => {
                self.msg = format!("{e:#} — usando as frases padrão");
                self.frases = dados::ler_frases(dados::FRASES_PADRAO).unwrap_or_default();
            }
        }
    }

    fn ideias(&self) -> Vec<String> {
        dados::ler_ideias(&std::fs::read_to_string(self.pasta.ideias()).unwrap_or_default())
    }

    /// O livro que "ir para o meu texto" abre.
    fn livro(&self) -> Option<PathBuf> {
        let raiz = self.pasta.raiz().filter(|r| crate::books::is_book(r));
        match (&self.ultimo_livro, raiz) {
            (Some(l), Some(r)) if l.starts_with(&r) && crate::books::is_book(l) => Some(l.clone()),
            (_, Some(r)) => Some(r),
            (Some(l), None) if crate::books::is_book(l) => Some(l.clone()),
            _ => None,
        }
    }

    fn ultimo_texto(&self) -> Option<PathBuf> {
        let r = dados::ultimo_texto(&self.pasta.registro())?;
        let p = PathBuf::from(&r);
        let p = if p.is_absolute() {
            p
        } else {
            self.pasta.raiz()?.join(p)
        };
        p.is_file().then_some(p)
    }

    // ── começar sessões ───────────────────────────────────────────────

    fn abrir_escolha_de_texto(&mut self) {
        let ultimo = self.ultimo_texto();
        let mut itens = dados::textos_para_continuar(&self.pasta);
        if let Some(u) = &ultimo {
            itens.retain(|p| p != u);
            itens.insert(0, u.clone());
        }
        self.tela = Tela::EscolherTexto {
            sel: 0,
            itens,
            ultimo,
        };
    }

    fn comecar_habito(&mut self, arquivo: PathBuf) {
        let existente = std::fs::read_to_string(&arquivo).unwrap_or_default();
        let base = dados::abrir_texto(&existente);
        let ultimo = dados::ultimo_paragrafo(&base.corpo);
        self.tela = Tela::Escrevendo(Box::new(Sessao::nova(
            Tipo::Habito {
                base,
                ultimo,
                timer: true,
                timer_avisado: false,
            },
            arquivo,
        )));
        self.msg.clear();
    }

    fn comecar_exercicio(&mut self, escolha: Option<usize>) {
        self.recarregar();
        if self.exercicios.is_empty() {
            self.msg = "exercicios.md não tem nenhum exercício (## Título)".into();
            return;
        }
        let aleatorio = escolha.is_none();
        let i = escolha.unwrap_or_else(|| self.sorteio.indice(self.exercicios.len()).unwrap());
        let item = self.exercicios[i.min(self.exercicios.len() - 1)].clone();
        let (sorteadas, sem_ideias) = self.sortear_ideias(item.ideias);
        let arquivo = dados::caminho_sessao(&self.pasta.sessoes(), Local::now(), &item.titulo);
        self.tela = Tela::Escrevendo(Box::new(Sessao::nova(
            Tipo::Exercicio {
                item,
                aleatorio,
                sorteadas,
                sem_ideias,
            },
            arquivo,
        )));
        self.msg.clear();
    }

    fn sortear_ideias(&mut self, k: usize) -> (Vec<String>, bool) {
        if k == 0 {
            return (vec![], false);
        }
        let ideias = self.ideias();
        if ideias.is_empty() {
            return (vec![], true);
        }
        let v = self
            .sorteio
            .distintos(ideias.len(), k)
            .into_iter()
            .map(|i| ideias[i].clone())
            .collect();
        (v, false)
    }

    fn comecar_frase(&mut self, escolha: Option<usize>) {
        if self.frases.is_empty() {
            self.msg = "frases.toml não tem nenhuma frase".into();
            return;
        }
        let aleatoria = escolha.is_none();
        let i = escolha.unwrap_or_else(|| self.sorteio.indice(self.frases.len()).unwrap());
        let frase = self.frases[i.min(self.frases.len() - 1)].clone();
        let nome = format!("frase {}", frase.autor);
        let arquivo = dados::caminho_sessao(&self.pasta.sessoes(), Local::now(), &nome);
        self.tela = Tela::Escrevendo(Box::new(Sessao::nova(
            Tipo::Frase { frase, aleatoria },
            arquivo,
        )));
        self.msg.clear();
    }

    fn ordem_frases(&self) -> Vec<usize> {
        let mut o: Vec<usize> = (0..self.frases.len()).collect();
        o.sort_by(|a, b| {
            let (fa, fb) = (&self.frases[*a], &self.frases[*b]);
            (fa.autor.to_lowercase(), fa.ano).cmp(&(fb.autor.to_lowercase(), fb.ano))
        });
        o
    }

    // ── terminar sessões ──────────────────────────────────────────────

    /// Salva e registra (uma vez só). Devolve a mensagem para a barra.
    fn encerrar(&mut self, s: &mut Sessao, bilhete: Option<&str>) -> String {
        let raiz = self.pasta.raiz();
        let salvo = match s.salvar(bilhete) {
            Ok(b) => b,
            Err(e) => return format!("não consegui salvar: {e:#} — o texto continua na tela"),
        };
        if !s.registrado {
            s.registrado = true;
            let arquivo = if salvo || !s.e_treino() {
                rel(raiz.as_deref(), &s.arquivo)
            } else {
                String::new()
            };
            let titulo = s.titulo();
            let r = dados::Registro {
                quando: s.quando,
                modo: s.modo(),
                atividade: &titulo,
                minutos: tempo::minutos(s.inicio.elapsed()),
                palavras: s.palavras(),
                arquivo: &arquivo,
            };
            if let Err(e) = dados::registrar(&self.pasta.registro(), &r) {
                return format!("salvo, mas o registro falhou: {e:#}");
            }
        }
        if salvo {
            format!("salvo em {}", rel(raiz.as_deref(), &s.arquivo))
        } else {
            "nada escrito, nada salvo".into()
        }
    }

    fn autosalvar(s: &mut Sessao) -> Option<String> {
        if s.editor.alterado && s.ultimo_salvo.elapsed() >= AUTOSALVAR {
            // No hábito, o bilhete só vem no fim; aqui salva só o texto.
            return s
                .salvar(None)
                .err()
                .map(|e| format!("não consegui salvar: {e:#}"));
        }
        None
    }

    /// Os relógios: aviso aos 25, limite aos 30, timer de 10 do hábito.
    fn relogios(&mut self) {
        let Tela::Escrevendo(s) = &mut self.tela else {
            return;
        };
        if let Some(m) = Self::autosalvar(s) {
            self.msg = m;
        }
        let dec = s.inicio.elapsed();
        if s.e_treino() {
            match tempo::fase(dec) {
                Fase::Aviso if !s.aviso_25_dado => {
                    s.aviso_25_dado = true;
                    sino();
                }
                Fase::Esgotado if !s.esgotado_visto && !s.modal_esgotado => {
                    s.aviso_25_dado = true;
                    s.modal_esgotado = true;
                    // Nada se perde: salva já, antes de perguntar qualquer coisa.
                    let _ = s.salvar(None);
                    sino();
                }
                _ => {}
            }
        } else if let Tipo::Habito {
            timer: true,
            timer_avisado,
            ..
        } = &mut s.tipo
            && !*timer_avisado
            && dec >= tempo::TIMER_HABITO
        {
            *timer_avisado = true;
            sino();
        }
    }

    fn salvar_tudo(&mut self) {
        let tela = std::mem::replace(&mut self.tela, Tela::Menu { sel: 0 });
        match tela {
            Tela::Escrevendo(mut s) => {
                let _ = self.encerrar(&mut s, None);
            }
            Tela::Bilhete {
                mut sessao,
                entrada,
            } => {
                let _ = self.encerrar(&mut sessao, Some(&entrada));
            }
            outra => self.tela = outra,
        }
    }

    // ── teclas ────────────────────────────────────────────────────────

    fn tecla(&mut self, k: KeyEvent, largura: usize) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(k.code, KeyCode::Char('q') | KeyCode::Char('c')) {
            self.salvar_tudo();
            self.saida = Some(Saida::Sair);
            return;
        }
        let tela = std::mem::replace(&mut self.tela, Tela::Menu { sel: 0 });
        self.tela = match tela {
            Tela::Menu { sel } => self.tecla_menu(k, sel),
            Tela::Dicas { rolagem } => match k.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Tela::Menu { sel: 3 },
                KeyCode::Down | KeyCode::Char('j') => Tela::Dicas {
                    rolagem: rolagem.saturating_add(1),
                },
                KeyCode::Up | KeyCode::Char('k') => Tela::Dicas {
                    rolagem: rolagem.saturating_sub(1),
                },
                KeyCode::PageDown | KeyCode::Char(' ') => Tela::Dicas {
                    rolagem: rolagem.saturating_add(10),
                },
                KeyCode::PageUp => Tela::Dicas {
                    rolagem: rolagem.saturating_sub(10),
                },
                _ => Tela::Dicas { rolagem },
            },
            Tela::EscolherTexto { sel, itens, ultimo } => {
                let n = itens.len() + 1;
                match k.code {
                    KeyCode::Esc => Tela::Menu { sel: 0 },
                    KeyCode::Up | KeyCode::Char('k') => Tela::EscolherTexto {
                        sel: sel.saturating_sub(1),
                        itens,
                        ultimo,
                    },
                    KeyCode::Down | KeyCode::Char('j') => Tela::EscolherTexto {
                        sel: (sel + 1).min(n - 1),
                        itens,
                        ultimo,
                    },
                    KeyCode::PageUp => Tela::EscolherTexto {
                        sel: sel.saturating_sub(10),
                        itens,
                        ultimo,
                    },
                    KeyCode::PageDown => Tela::EscolherTexto {
                        sel: (sel + 10).min(n - 1),
                        itens,
                        ultimo,
                    },
                    KeyCode::Enter if sel == 0 => Tela::NomeTextoNovo {
                        entrada: String::new(),
                    },
                    KeyCode::Enter => {
                        self.comecar_habito(itens[sel - 1].clone());
                        return;
                    }
                    _ => Tela::EscolherTexto { sel, itens, ultimo },
                }
            }
            Tela::NomeTextoNovo { mut entrada } => match k.code {
                KeyCode::Esc => {
                    self.abrir_escolha_de_texto();
                    return;
                }
                KeyCode::Enter if !entrada.trim().is_empty() => {
                    let mut p = self
                        .pasta
                        .textos()
                        .join(format!("{}.md", dados::slug(&entrada)));
                    let mut n = 2;
                    while p.exists() {
                        p = self
                            .pasta
                            .textos()
                            .join(format!("{}-{n}.md", dados::slug(&entrada)));
                        n += 1;
                    }
                    self.comecar_habito(p);
                    return;
                }
                KeyCode::Backspace => {
                    entrada.pop();
                    Tela::NomeTextoNovo { entrada }
                }
                KeyCode::Char(c) if !ctrl => {
                    entrada.push(c);
                    Tela::NomeTextoNovo { entrada }
                }
                _ => Tela::NomeTextoNovo { entrada },
            },
            Tela::ListaExercicios { sel } => {
                let n = self.exercicios.len() + 1;
                match k.code {
                    KeyCode::Esc => Tela::Menu { sel: 1 },
                    KeyCode::Up | KeyCode::Char('k') => Tela::ListaExercicios {
                        sel: sel.saturating_sub(1),
                    },
                    KeyCode::Down | KeyCode::Char('j') => Tela::ListaExercicios {
                        sel: (sel + 1).min(n - 1),
                    },
                    KeyCode::Char('a') => {
                        self.comecar_exercicio(None);
                        return;
                    }
                    KeyCode::Char(c @ '1'..='9') if (c as usize - '0' as usize) < n => {
                        self.comecar_exercicio(Some(c as usize - '1' as usize));
                        return;
                    }
                    KeyCode::Enter => {
                        self.comecar_exercicio(if sel == 0 { None } else { Some(sel - 1) });
                        return;
                    }
                    _ => Tela::ListaExercicios { sel },
                }
            }
            Tela::ListaFrases { sel, ordem } => {
                let n = ordem.len() + 1;
                match k.code {
                    KeyCode::Esc => Tela::Menu { sel: 2 },
                    KeyCode::Up | KeyCode::Char('k') => Tela::ListaFrases {
                        sel: sel.saturating_sub(1),
                        ordem,
                    },
                    KeyCode::Down | KeyCode::Char('j') => Tela::ListaFrases {
                        sel: (sel + 1).min(n - 1),
                        ordem,
                    },
                    KeyCode::PageUp => Tela::ListaFrases {
                        sel: sel.saturating_sub(10),
                        ordem,
                    },
                    KeyCode::PageDown => Tela::ListaFrases {
                        sel: (sel + 10).min(n - 1),
                        ordem,
                    },
                    KeyCode::Char('a') => {
                        self.comecar_frase(None);
                        return;
                    }
                    KeyCode::Enter => {
                        self.comecar_frase(if sel == 0 { None } else { Some(ordem[sel - 1]) });
                        return;
                    }
                    _ => Tela::ListaFrases { sel, ordem },
                }
            }
            Tela::Escrevendo(s) => return self.tecla_escrevendo(k, s, largura),
            Tela::Bilhete {
                mut sessao,
                mut entrada,
            } => match k.code {
                KeyCode::Esc => {
                    // volta a escrever, sem perder nada
                    Tela::Escrevendo(sessao)
                }
                KeyCode::Enter => {
                    self.msg = self.encerrar(&mut sessao, Some(&entrada));
                    Tela::Menu { sel: 0 }
                }
                KeyCode::Backspace => {
                    entrada.pop();
                    Tela::Bilhete { sessao, entrada }
                }
                KeyCode::Char(c) if !ctrl => {
                    entrada.push(c);
                    Tela::Bilhete { sessao, entrada }
                }
                _ => Tela::Bilhete { sessao, entrada },
            },
        };
    }

    fn tecla_menu(&mut self, k: KeyEvent, sel: usize) -> Tela {
        const N: usize = 6;
        let escolha = match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                return Tela::Menu {
                    sel: sel.saturating_sub(1),
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                return Tela::Menu {
                    sel: (sel + 1).min(N - 1),
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => 5,
            KeyCode::Enter => sel,
            KeyCode::Char(c @ '1'..='6') => c as usize - '1' as usize,
            KeyCode::Char('h') => 0,
            KeyCode::Char('e') => 1,
            KeyCode::Char('f') => 2,
            KeyCode::Char('d') => 3,
            KeyCode::Char('g') => 4,
            _ => return Tela::Menu { sel },
        };
        match escolha {
            0 => {
                self.abrir_escolha_de_texto();
                std::mem::replace(&mut self.tela, Tela::Menu { sel: 0 })
            }
            1 => {
                self.recarregar();
                Tela::ListaExercicios { sel: 0 }
            }
            2 => {
                self.recarregar();
                Tela::ListaFrases {
                    sel: 0,
                    ordem: self.ordem_frases(),
                }
            }
            3 => {
                self.recarregar();
                Tela::Dicas { rolagem: 0 }
            }
            4 => {
                self.saida = Some(Saida::AbrirLivro(self.livro()));
                Tela::Menu { sel }
            }
            _ => {
                self.saida = Some(Saida::Sair);
                Tela::Menu { sel }
            }
        }
    }

    fn tecla_escrevendo(&mut self, k: KeyEvent, mut s: Box<Sessao>, largura: usize) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);

        if s.modal_esgotado {
            match k.code {
                KeyCode::Enter | KeyCode::Char('t') => {
                    self.msg = self.encerrar(&mut s, None);
                    self.saida = Some(Saida::AbrirLivro(self.livro()));
                    self.tela = Tela::Menu { sel: 0 };
                }
                KeyCode::Char('h') => {
                    self.msg = self.encerrar(&mut s, None);
                    match self.ultimo_texto() {
                        Some(p) => self.comecar_habito(p),
                        None => self.abrir_escolha_de_texto(),
                    }
                }
                KeyCode::Char('m') => {
                    self.msg = self.encerrar(&mut s, None);
                    self.tela = Tela::Menu { sel: 0 };
                }
                KeyCode::Esc => {
                    s.modal_esgotado = false;
                    s.esgotado_visto = true;
                    self.tela = Tela::Escrevendo(s);
                }
                _ => self.tela = Tela::Escrevendo(s),
            }
            return;
        }

        match k.code {
            KeyCode::Esc => {
                if s.e_treino() {
                    self.msg = self.encerrar(&mut s, None);
                    self.tela = Tela::Menu { sel: 0 };
                } else {
                    let entrada = match &s.tipo {
                        Tipo::Habito { base, .. } => base.bilhete.clone().unwrap_or_default(),
                        _ => String::new(),
                    };
                    self.tela = Tela::Bilhete { sessao: s, entrada };
                }
                return;
            }
            KeyCode::Char('s') if ctrl => {
                self.msg = match s.salvar(None) {
                    Ok(true) => {
                        format!("salvo em {}", rel(self.pasta.raiz().as_deref(), &s.arquivo))
                    }
                    Ok(false) => "nada escrito ainda".into(),
                    Err(e) => format!("não consegui salvar: {e:#}"),
                };
            }
            KeyCode::Char('g') if ctrl => {
                self.msg = self.encerrar(&mut s, None);
                self.saida = Some(Saida::AbrirLivro(self.livro()));
                self.tela = Tela::Menu { sel: 0 };
                return;
            }
            KeyCode::F(2) => {
                if let Tipo::Habito { timer, .. } = &mut s.tipo {
                    *timer = !*timer;
                    self.msg = if *timer {
                        "timer de 10 minutos ligado".into()
                    } else {
                        "timer desligado".into()
                    };
                }
            }
            KeyCode::F(5) => {
                let k = match &s.tipo {
                    Tipo::Exercicio { item, .. } => item.ideias,
                    _ => 0,
                };
                if k > 0 {
                    let (v, vazio) = self.sortear_ideias(k);
                    if let Tipo::Exercicio {
                        sorteadas,
                        sem_ideias,
                        ..
                    } = &mut s.tipo
                    {
                        *sorteadas = v;
                        *sem_ideias = vazio;
                    }
                }
            }
            KeyCode::Enter => s.editor.inserir('\n'),
            KeyCode::Backspace if ctrl || alt => s.editor.apagar_palavra(),
            KeyCode::Char('w') if ctrl => s.editor.apagar_palavra(),
            KeyCode::Char('h') if ctrl => s.editor.apagar_palavra(),
            KeyCode::Backspace => s.editor.apagar_antes(),
            KeyCode::Delete => s.editor.apagar_depois(),
            KeyCode::Left if ctrl || alt => s.editor.palavra_esquerda(),
            KeyCode::Right if ctrl || alt => s.editor.palavra_direita(),
            KeyCode::Left => s.editor.esquerda(),
            KeyCode::Right => s.editor.direita(),
            KeyCode::Up => s.editor.acima(largura),
            KeyCode::Down => s.editor.abaixo(largura),
            KeyCode::Home => s.editor.inicio_linha(largura),
            KeyCode::End => s.editor.fim_linha(largura),
            KeyCode::Tab => s.editor.inserir('\t'),
            KeyCode::Char(c) if !ctrl || k.modifiers.contains(KeyModifiers::ALT) => {
                s.editor.inserir(c)
            }
            _ => {}
        }
        self.tela = Tela::Escrevendo(s);
    }

    fn colar(&mut self, texto: &str) {
        match &mut self.tela {
            Tela::Escrevendo(s) if !s.modal_esgotado => s.editor.colar(texto),
            Tela::NomeTextoNovo { entrada } | Tela::Bilhete { entrada, .. } => {
                entrada.extend(texto.chars().filter(|c| !c.is_control()))
            }
            _ => {}
        }
    }
}

// ── desenho ───────────────────────────────────────────────────────────

fn coluna(area: Rect, max: u16) -> Rect {
    let w = area.width.min(max);
    Rect {
        x: area.x + (area.width - w) / 2,
        width: w,
        ..area
    }
}

fn fraco() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn negrito() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn desenhar_lista(
    f: &mut Frame,
    area: Rect,
    titulo: &str,
    subtitulo: &str,
    itens: &[(String, String)],
    sel: usize,
    rodape: &str,
) {
    let area = coluna(area, LARGURA_MAX + 4);
    let [cab, corpo, pe] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    f.render_widget(
        Paragraph::new(vec![
            Line::styled(titulo.to_string(), negrito()),
            Line::styled(subtitulo.to_string(), fraco()),
        ]),
        Rect {
            y: cab.y + 1,
            height: 2,
            ..cab
        },
    );
    let h = corpo.height as usize;
    let topo = if sel >= h { sel + 1 - h } else { 0 };
    let linhas: Vec<Line> = itens
        .iter()
        .enumerate()
        .skip(topo)
        .take(h)
        .map(|(i, (a, b))| {
            let marca = if i == sel { "› " } else { "  " };
            let estilo = if i == sel {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let mut v = vec![Span::styled(format!("{marca}{a}"), estilo)];
            if !b.is_empty() {
                v.push(Span::styled(format!("  {b}"), fraco()));
            }
            Line::from(v)
        })
        .collect();
    f.render_widget(Paragraph::new(linhas), corpo);
    f.render_widget(
        Paragraph::new(Line::styled(rodape.to_string(), fraco())),
        pe,
    );
}

fn desenhar(f: &mut Frame, t: &mut Treino) {
    let area = f.area();
    let [principal, barra] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let raiz = t.pasta.raiz();
    match &mut t.tela {
        Tela::Menu { sel } => {
            let col = coluna(principal, 72);
            let itens = [
                (
                    "1  Sessão de hábito",
                    "continuar um texto · timer de 10 min",
                ),
                ("2  Exercícios", "escolha um, ou Aleatório · até 30 min"),
                ("3  Continue a frase", "abertura de um livro · até 30 min"),
                ("4  Dicas", "as 7 dicas para criar o hábito"),
                ("5  Ir para o meu texto", "abre o Grimoire no seu livro"),
                ("6  Sair", ""),
            ];
            let mut linhas = vec![
                Line::raw(""),
                Line::styled("Grimoire · Treino", negrito()),
                Line::styled(
                    "Treine no máximo 30 minutos, depois vá para o seu texto.",
                    fraco(),
                ),
                Line::raw(""),
            ];
            for (i, (a, b)) in itens.iter().enumerate() {
                let est = if i == *sel {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                linhas.push(Line::from(vec![
                    Span::styled(format!("{} {a}", if i == *sel { "›" } else { " " }), est),
                    Span::styled(
                        if b.is_empty() {
                            String::new()
                        } else {
                            format!("   {b}")
                        },
                        fraco(),
                    ),
                ]));
                linhas.push(Line::raw(""));
            }
            linhas.push(Line::styled(
                format!("Arquivos em {}", t.pasta.dir.display()),
                fraco(),
            ));
            f.render_widget(Paragraph::new(linhas).wrap(Wrap { trim: false }), col);
        }
        Tela::Dicas { rolagem } => {
            let col = coluna(principal, LARGURA_MAX);
            let mut linhas = vec![
                Line::raw(""),
                Line::styled("Dicas para criar o hábito", negrito()),
                Line::raw(""),
            ];
            for (i, d) in t.dicas.iter().enumerate() {
                linhas.push(Line::styled(format!("{}. {}", i + 1, d.titulo), negrito()));
                linhas.push(Line::raw(d.texto.clone()));
                if let Some(m) = &d.meta {
                    linhas.push(Line::styled(format!("Meta: {m}"), fraco()));
                }
                linhas.push(Line::raw(""));
            }
            linhas.push(Line::styled("↑↓ rolar · Esc voltar", fraco()));
            f.render_widget(
                Paragraph::new(linhas)
                    .wrap(Wrap { trim: false })
                    .scroll((*rolagem, 0)),
                col,
            );
        }
        Tela::EscolherTexto { sel, itens, ultimo } => {
            let mut v = vec![(
                "Texto novo…".to_string(),
                "começa um arquivo em exercicios/textos/".to_string(),
            )];
            for p in itens.iter() {
                let marca = if Some(p) == ultimo.as_ref() {
                    "último"
                } else {
                    ""
                };
                v.push((rel(raiz.as_deref(), p), marca.to_string()));
            }
            desenhar_lista(
                f,
                principal,
                "Sessão de hábito — qual texto?",
                "Você verá só o último parágrafo e o seu bilhete [Próximo: …].",
                &v,
                *sel,
                "Enter começar · Esc voltar · (o timer de 10 min liga/desliga com F2 durante a sessão)",
            );
        }
        Tela::NomeTextoNovo { entrada } => {
            let col = coluna(principal, LARGURA_MAX);
            let linhas = vec![
                Line::raw(""),
                Line::styled("Texto novo", negrito()),
                Line::styled(
                    "Dê um nome (vira o nome do arquivo em exercicios/textos/).",
                    fraco(),
                ),
                Line::raw(""),
                Line::raw(format!("Nome: {entrada}▏")),
                Line::raw(""),
                Line::styled("Enter começar · Esc voltar", fraco()),
            ];
            f.render_widget(Paragraph::new(linhas), col);
        }
        Tela::ListaExercicios { sel } => {
            let mut v = vec![("Aleatório".to_string(), "sorteia um exercício".to_string())];
            for (i, e) in t.exercicios.iter().enumerate() {
                v.push((
                    format!("{}. {}", i + 1, e.titulo),
                    e.meta
                        .clone()
                        .map(|m| format!("meta: {m}"))
                        .unwrap_or_default(),
                ));
            }
            desenhar_lista(
                f,
                principal,
                "Exercícios",
                "Escolha um ou sorteie. Treino de no máximo 30 minutos.",
                &v,
                *sel,
                "Enter começar · a Aleatório · 1–9 atalho · Esc voltar",
            );
        }
        Tela::ListaFrases { sel, ordem } => {
            let mut v = vec![("Aleatório".to_string(), "sorteia uma abertura".to_string())];
            for i in ordem.iter() {
                let fr = &t.frases[*i];
                v.push((
                    format!("{} — {}", fr.autor, fr.obra),
                    fr.ano.map(|a| a.to_string()).unwrap_or_default(),
                ));
            }
            desenhar_lista(
                f,
                principal,
                "Continue a frase",
                "Escolha um autor e uma obra, ou sorteie. Treino de no máximo 30 minutos.",
                &v,
                *sel,
                "Enter começar · a Aleatório · Esc voltar",
            );
        }
        Tela::Escrevendo(s) => desenhar_escrita(f, principal, s),
        Tela::Bilhete { sessao, entrada } => {
            desenhar_escrita(f, principal, sessao);
            let col = coluna(principal, 70);
            let caixa = Rect {
                y: principal.y + principal.height / 3,
                height: 8,
                ..col
            };
            f.render_widget(Clear, caixa);
            let linhas = vec![
                Line::styled("Bilhete para a próxima vez", negrito()),
                Line::styled(
                    "Uma linha sobre o que vem a seguir. Fica no topo do texto.",
                    fraco(),
                ),
                Line::raw(""),
                Line::raw(format!("[Próximo: {entrada}▏]")),
                Line::raw(""),
                Line::styled("Enter salvar e terminar · Esc voltar a escrever", fraco()),
            ];
            f.render_widget(
                Paragraph::new(linhas).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .padding(Padding::horizontal(1)),
                ),
                caixa,
            );
        }
    }
    let msg = if t.msg.is_empty() {
        String::new()
    } else {
        format!(" {}", t.msg)
    };
    f.render_widget(Paragraph::new(Line::styled(msg, fraco())), barra);
}

fn desenhar_escrita(f: &mut Frame, area: Rect, s: &mut Sessao) {
    let col = coluna(area, LARGURA_MAX);
    let largura = col.width.max(10) as usize;
    let dec = s.inicio.elapsed();

    // Cabeçalho: o que fazer, a meta, o relógio.
    let mut cab: Vec<Line> = Vec::new();
    let mut relogio: Vec<Span> = Vec::new();
    match &s.tipo {
        Tipo::Habito {
            base,
            ultimo,
            timer,
            ..
        } => {
            cab.push(Line::styled("Sessão de hábito", negrito()));
            if let Some(b) = &base.bilhete {
                cab.push(Line::styled(
                    format!("[Próximo: {b}]"),
                    Style::default().fg(Color::Yellow),
                ));
            }
            if let Some(u) = ultimo {
                cab.push(Line::styled(
                    "Redigite o último parágrafo para aquecer:",
                    fraco(),
                ));
                let mut txt = u.replace('\n', " ");
                if txt.chars().count() > 600 {
                    txt = format!(
                        "…{}",
                        txt.chars()
                            .rev()
                            .take(600)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect::<String>()
                    );
                }
                cab.push(Line::styled(txt, fraco().add_modifier(Modifier::ITALIC)));
            } else {
                cab.push(Line::styled("Texto novo. Comece por onde quiser.", fraco()));
            }
            if *timer {
                if dec >= tempo::TIMER_HABITO {
                    relogio.push(Span::styled(
                        format!(
                            "Timer: {} · {}",
                            tempo::MSG_TIMER_FIM,
                            tempo::mmss(dec - tempo::TIMER_HABITO)
                        ),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    relogio.push(Span::styled(
                        format!(
                            "Timer: {} de 10:00",
                            tempo::mmss(tempo::restante(tempo::TIMER_HABITO, dec))
                        ),
                        fraco(),
                    ));
                }
            } else {
                relogio.push(Span::styled(
                    format!("{} escrevendo", tempo::mmss(dec)),
                    fraco(),
                ));
            }
        }
        Tipo::Exercicio {
            item,
            aleatorio,
            sorteadas,
            sem_ideias,
        } => {
            let pre = if *aleatorio {
                "Exercício (sorteado)"
            } else {
                "Exercício"
            };
            cab.push(Line::styled(format!("{pre}: {}", item.titulo), negrito()));
            cab.push(Line::raw(item.texto.clone()));
            if let Some(m) = &item.meta {
                cab.push(Line::styled(
                    format!("Meta: {m}"),
                    Style::default().fg(Color::Cyan),
                ));
            }
            if *sem_ideias {
                cab.push(Line::styled(
                    "Seu ideias.md está vazio — anote ideias em exercicios/ideias.md, uma por linha. Por ora, use uma ideia da cabeça.",
                    Style::default().fg(Color::Yellow),
                ));
            }
            for (i, ideia) in sorteadas.iter().enumerate() {
                let rot = if sorteadas.len() > 1 {
                    format!("Ideia {}: ", i + 1)
                } else {
                    "Ideia sorteada: ".into()
                };
                cab.push(Line::from(vec![
                    Span::styled(rot, fraco()),
                    Span::styled(ideia.clone(), negrito()),
                ]));
            }
        }
        Tipo::Frase { frase, aleatoria } => {
            let pre = if *aleatoria {
                "Continue a frase (sorteada)"
            } else {
                "Continue a frase"
            };
            cab.push(Line::styled(pre, negrito()));
            cab.push(Line::styled(
                format!("“{}”", frase.texto),
                Style::default().add_modifier(Modifier::ITALIC),
            ));
            cab.push(Line::styled(format!("— {}", frase.credito()), fraco()));
        }
    }
    if s.e_treino() {
        let (txt, est) = match tempo::fase(dec) {
            Fase::Normal => (
                format!(
                    "Treino: {} restantes",
                    tempo::mmss(tempo::restante(tempo::LIMITE_TREINO, dec))
                ),
                fraco(),
            ),
            Fase::Aviso => (
                format!(
                    "{} · {}",
                    tempo::MSG_AVISO,
                    tempo::mmss(tempo::restante(tempo::LIMITE_TREINO, dec))
                ),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Fase::Esgotado => (
                format!(
                    "{} · +{}",
                    tempo::MSG_ESGOTADO,
                    tempo::mmss(dec - tempo::LIMITE_TREINO)
                ),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        relogio.push(Span::styled(txt, est));
    }
    relogio.push(Span::styled(
        format!("  ·  {} palavras", s.palavras()),
        fraco(),
    ));
    cab.push(Line::from(relogio));

    // Altura do cabeçalho com a quebra de linha, no máximo 45% da tela.
    let alt_cab: usize = cab
        .iter()
        .map(|l| {
            let n: usize = l.spans.iter().map(|sp| sp.content.chars().count()).sum();
            n.div_ceil(largura).max(1)
        })
        .sum::<usize>()
        + 1;
    let alt_cab = (alt_cab as u16).min(area.height * 45 / 100).max(2);
    let [c_area, sep, e_area, ajuda] = Layout::vertical([
        Constraint::Length(alt_cab),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(col);
    f.render_widget(Paragraph::new(cab).wrap(Wrap { trim: false }), c_area);
    f.render_widget(
        Paragraph::new(Line::styled("─".repeat(col.width as usize), fraco())),
        sep,
    );

    // O texto.
    let w = e_area.width.max(1) as usize;
    let h = e_area.height as usize;
    s.editor.ajustar_rolagem(w, h);
    let linhas = quebrar(&s.editor.texto, w);
    let visiveis: Vec<Line> = linhas
        .iter()
        .skip(s.editor.rolagem)
        .take(h)
        .map(|l| Line::raw(s.editor.texto[l.inicio..l.fim].iter().collect::<String>()))
        .collect();
    f.render_widget(Paragraph::new(visiveis), e_area);
    let (r, c) = s.editor.posicao(&linhas);
    if !s.modal_esgotado && r >= s.editor.rolagem && r < s.editor.rolagem + h {
        f.set_cursor_position((
            e_area.x + (c.min(w.saturating_sub(1))) as u16,
            e_area.y + (r - s.editor.rolagem) as u16,
        ));
    }

    let dica = match s.tipo {
        Tipo::Habito { .. } => {
            "Esc terminar (pede o bilhete) · Ctrl-S salvar · F2 timer · Ctrl-G Grimoire"
        }
        Tipo::Exercicio { ref item, .. } if item.ideias > 0 => {
            "Esc terminar · Ctrl-S salvar · F5 outra ideia · Ctrl-G ir para o meu texto"
        }
        _ => "Esc terminar e salvar · Ctrl-S salvar · Ctrl-G ir para o meu texto",
    };
    f.render_widget(Paragraph::new(Line::styled(dica, fraco())), ajuda);

    if s.modal_esgotado {
        let caixa_col = coluna(area, 66);
        let caixa = Rect {
            y: area.y + area.height / 4,
            height: 11.min(area.height),
            ..caixa_col
        };
        f.render_widget(Clear, caixa);
        let linhas = vec![
            Line::styled(
                tempo::MSG_ESGOTADO,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::raw("Seu treino já foi salvo. Nada do que você escreveu se perde."),
            Line::raw(""),
            Line::raw("Enter  ir para o meu texto (abre o Grimoire)"),
            Line::raw("h      sessão de hábito no último texto"),
            Line::raw("m      voltar ao menu"),
            Line::styled(
                "Esc    só terminar a frase (o relógio fica vermelho)",
                fraco(),
            ),
        ];
        f.render_widget(
            Paragraph::new(linhas).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
                    .padding(Padding::horizontal(1)),
            ),
            caixa,
        );
    }
}

/// Largura da página de escrever, para as setas ↑↓ saberem onde quebra.
fn largura_texto(area: Rect) -> usize {
    coluna(area, LARGURA_MAX).width.max(1) as usize
}

/// Roda o Treino no terminal já aberto pelo Grimoire.
pub fn run(
    terminal: &mut ratatui::DefaultTerminal,
    cfg: Config,
    eventos: &std::sync::mpsc::Receiver<std::io::Result<Event>>,
) -> Result<Saida> {
    let mut t = Treino::new(&cfg);
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Saida> {
        loop {
            if crate::shutdown::requested() {
                t.salvar_tudo();
                return Ok(Saida::Sair);
            }
            t.relogios();
            terminal.draw(|f| desenhar(f, &mut t))?;
            if let Some(s) = t.saida.take() {
                return Ok(s);
            }
            let ev = match eventos.recv_timeout(Duration::from_millis(250)) {
                Ok(ev) => ev?,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    t.salvar_tudo();
                    return Ok(Saida::Sair);
                }
            };
            let largura = largura_texto(terminal.get_frame().area());
            match ev {
                Event::Key(k) if k.kind == KeyEventKind::Press => t.tecla(k, largura),
                Event::Paste(p) => t.colar(&p),
                _ => {}
            }
            if let Some(s) = t.saida.take() {
                return Ok(s);
            }
        }
    }));
    match res {
        Ok(r) => {
            if r.is_err() {
                t.salvar_tudo();
            }
            r
        }
        Err(_) => {
            // Um erro do programa: guarda o que foi escrito antes de sair.
            t.salvar_tudo();
            Err(anyhow::anyhow!(
                "o Treino encontrou um erro e fechou; o que você escreveu foi salvo"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn pasta_temp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("treino-fluxo-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("manuscript")).unwrap();
        d
    }

    fn treino(d: &Path) -> Treino {
        Treino::new(&Config {
            pedida: Some(d.to_path_buf()),
            ultimo_livro: None,
            reserva: d.join("reserva"),
        })
    }

    fn tela(t: &mut Treino) -> String {
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| desenhar(f, t)).unwrap();
        let b = term.backend().buffer().clone();
        let mut s = String::new();
        for y in 0..b.area.height {
            for x in 0..b.area.width {
                s.push_str(b[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    fn k(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn digitar(t: &mut Treino, s: &str) {
        for c in s.chars() {
            t.tecla(
                k(if c == '\n' {
                    KeyCode::Enter
                } else {
                    KeyCode::Char(c)
                }),
                70,
            );
        }
    }

    fn adiantar(t: &mut Treino, minutos: u64) {
        if let Tela::Escrevendo(s) = &mut t.tela {
            s.inicio = Instant::now() - Duration::from_secs(minutos * 60);
        }
    }

    #[test]
    fn menu_em_portugues_e_arquivos_criados() {
        let d = pasta_temp("menu");
        let mut t = treino(&d);
        let s = tela(&mut t);
        for w in [
            "Sessão de hábito",
            "Exercícios",
            "Continue a frase",
            "Ir para o meu texto",
        ] {
            assert!(s.contains(w), "{w} não aparece:\n{s}");
        }
        assert!(d.join("exercicios/frases.toml").is_file());
        assert!(d.join("exercicios/ideias.md").is_file());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn exercicio_com_aviso_aos_25_e_limite_aos_30_sem_perder_nada() {
        let d = pasta_temp("limite");
        let mut t = treino(&d);
        t.tecla(k(KeyCode::Char('2')), 70);
        assert!(tela(&mut t).contains("Aleatório"));
        t.tecla(k(KeyCode::Char('3')), 70); // "Escreva a cena…"
        let s = tela(&mut t);
        assert!(s.contains("Meta: uma cena"), "{s}");
        assert!(
            s.contains("Treino: 30:00 restantes") || s.contains("Treino: 29:59 restantes"),
            "{s}"
        );
        digitar(&mut t, "A chuva caiu sobre a casa.");
        adiantar(&mut t, 25);
        t.relogios();
        assert!(tela(&mut t).contains(tempo::MSG_AVISO));
        adiantar(&mut t, 30);
        t.relogios();
        let s = tela(&mut t);
        assert!(s.contains(tempo::MSG_ESGOTADO), "{s}");
        // Já salvo, antes de qualquer escolha.
        let sessoes: Vec<_> = std::fs::read_dir(d.join("exercicios/sessoes"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(sessoes.len(), 1);
        let conteudo = std::fs::read_to_string(sessoes[0].path()).unwrap();
        assert!(conteudo.contains("A chuva caiu sobre a casa."));
        assert!(conteudo.contains("# Exercício: Escreva a cena que você está louco para escrever"));
        // Esc deixa terminar a frase, e o texto continua.
        t.tecla(k(KeyCode::Esc), 70);
        digitar(&mut t, " Fim.");
        assert!(!tela(&mut t).contains("Enter  ir para o meu texto"));
        // Enter-less exit: Ctrl-G salva e vai para o livro.
        t.tecla(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL), 70);
        assert_eq!(t.saida, Some(Saida::AbrirLivro(Some(d.clone()))));
        let conteudo = std::fs::read_to_string(sessoes[0].path()).unwrap();
        assert!(conteudo.contains("A chuva caiu sobre a casa. Fim."));
        let reg = std::fs::read_to_string(d.join("exercicios/registro.csv")).unwrap();
        assert_eq!(reg.lines().count(), 2, "uma sessão, uma linha:\n{reg}");
        assert!(reg.lines().nth(1).unwrap().contains(",exercicio,"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn roleta_sem_ideias_avisa_e_com_ideias_sorteia() {
        let d = pasta_temp("roleta");
        let mut t = treino(&d);
        t.tecla(k(KeyCode::Char('2')), 70);
        t.tecla(k(KeyCode::Char('1')), 70);
        assert!(tela(&mut t).contains("ideias.md está vazio"));
        t.tecla(k(KeyCode::Esc), 70);
        assert!(
            !d.join("exercicios/sessoes").exists()
                || std::fs::read_dir(d.join("exercicios/sessoes"))
                    .unwrap()
                    .next()
                    .is_none(),
            "treino vazio não cria arquivo"
        );
        std::fs::write(
            d.join("exercicios/ideias.md"),
            "o farol apagado\numa carta sem selo\n",
        )
        .unwrap();
        t.tecla(k(KeyCode::Char('2')), 70);
        t.tecla(k(KeyCode::Char('6')), 70); // Colisão: duas ideias
        let s = tela(&mut t);
        assert!(s.contains("Ideia 1:") && s.contains("Ideia 2:"), "{s}");
        assert!(s.contains("o farol apagado") && s.contains("uma carta sem selo"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn frase_aleatoria_mostra_autor_e_obra() {
        let d = pasta_temp("frase");
        let mut t = treino(&d);
        t.tecla(k(KeyCode::Char('3')), 70);
        t.tecla(k(KeyCode::Char('a')), 70);
        let Tela::Escrevendo(s) = &t.tela else {
            panic!("não começou")
        };
        let Tipo::Frase { frase, aleatoria } = &s.tipo else {
            panic!()
        };
        assert!(*aleatoria);
        let (autor, obra) = (frase.autor.clone(), frase.obra.clone());
        let s = tela(&mut t);
        assert!(s.contains(&autor) && s.contains("Continue a frase"), "{s}");
        let _ = obra;
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn habito_mostra_bilhete_e_ultimo_paragrafo_e_grava_o_novo_bilhete() {
        let d = pasta_temp("habito");
        let cena = d.join("manuscript/cena.md");
        std::fs::write(&cena, "---\ntitle: Cena\n---\n\n[Próximo: ela acha a carta]\n\nPrimeiro parágrafo.\n\nEla abriu a gaveta e viu o envelope.\n").unwrap();
        let mut t = treino(&d);
        t.tecla(k(KeyCode::Char('1')), 70);
        t.tecla(k(KeyCode::Down), 70);
        t.tecla(k(KeyCode::Enter), 70);
        let s = tela(&mut t);
        assert!(s.contains("[Próximo: ela acha a carta]"), "{s}");
        assert!(s.contains("Ela abriu a gaveta e viu o envelope."));
        assert!(
            !s.contains("Primeiro parágrafo."),
            "não mostra o texto inteiro"
        );
        assert!(s.contains("10:00"));
        digitar(
            &mut t,
            "Ela abriu a gaveta e viu o envelope.\nDentro, uma foto.",
        );
        adiantar(&mut t, 10);
        t.relogios();
        assert!(tela(&mut t).contains(tempo::MSG_TIMER_FIM));
        t.tecla(k(KeyCode::Esc), 70);
        assert!(tela(&mut t).contains("Bilhete para a próxima vez"));
        for _ in 0..40 {
            t.tecla(k(KeyCode::Backspace), 70);
        }
        digitar(&mut t, "quem tirou a foto?");
        t.tecla(k(KeyCode::Enter), 70);
        let out = std::fs::read_to_string(&cena).unwrap();
        assert_eq!(
            out,
            "---\ntitle: Cena\n---\n\n[Próximo: quem tirou a foto?]\n\nPrimeiro parágrafo.\n\nEla abriu a gaveta e viu o envelope.\n\nDentro, uma foto.\n"
        );
        let reg = std::fs::read_to_string(d.join("exercicios/registro.csv")).unwrap();
        assert!(reg.contains(",habito,Sessão de hábito,10,"), "{reg}");
        assert!(reg.trim_end().ends_with("manuscript/cena.md"));
        assert_eq!(t.ultimo_texto(), Some(cena.clone()));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
