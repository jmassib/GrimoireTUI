//! Os relógios do modo Treino: o limite de 30 minutos dos exercícios, o
//! timer opcional de 10 minutos da sessão de hábito, e o sorteio.

use std::time::Duration;

/// Duração máxima de um treino (exercício ou Continue a frase).
pub const LIMITE_TREINO: Duration = Duration::from_secs(30 * 60);
/// Quando aparece o aviso de que faltam 5 minutos.
pub const AVISO_TREINO: Duration = Duration::from_secs(25 * 60);
/// O timer da sessão de hábito ("Dez minutos, nada mais").
pub const TIMER_HABITO: Duration = Duration::from_secs(10 * 60);

pub const MSG_AVISO: &str = "Faltam 5 minutos — prepare-se para ir para o texto";
pub const MSG_ESGOTADO: &str = "Tempo de treino acabou — vá para o seu texto";
pub const MSG_TIMER_FIM: &str = "Pode parar — ou continue";

/// Em que ponto do treino estamos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Normal,
    /// De 25 a 30 minutos.
    Aviso,
    /// 30 minutos ou mais.
    Esgotado,
}

pub fn fase(decorrido: Duration) -> Fase {
    if decorrido >= LIMITE_TREINO {
        Fase::Esgotado
    } else if decorrido >= AVISO_TREINO {
        Fase::Aviso
    } else {
        Fase::Normal
    }
}

/// Quanto falta para o limite (zero depois dele).
pub fn restante(limite: Duration, decorrido: Duration) -> Duration {
    limite.saturating_sub(decorrido)
}

/// "mm:ss" (ou "h:mm:ss" acima de uma hora).
pub fn mmss(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Minutos inteiros, arredondados, para o registro.
pub fn minutos(d: Duration) -> u64 {
    (d.as_secs() + 30) / 60
}

/// Um gerador pequeno e previsível (xorshift64*), para não depender de
/// outra biblioteca. Semeado pelo relógio no app e por um número nos testes.
#[derive(Debug, Clone)]
pub struct Sorteio(u64);

impl Sorteio {
    pub fn new(semente: u64) -> Self {
        Sorteio(semente.max(1) ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn do_relogio() -> Self {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42);
        Sorteio::new(n ^ (std::process::id() as u64).rotate_left(32))
    }

    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Um índice em 0..n, ou None se não há nada para sortear.
    pub fn indice(&mut self, n: usize) -> Option<usize> {
        if n == 0 {
            None
        } else {
            Some((self.proximo() % n as u64) as usize)
        }
    }

    /// `k` índices diferentes em 0..n (menos, se n < k).
    pub fn distintos(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut todos: Vec<usize> = (0..n).collect();
        let mut out = Vec::new();
        while out.len() < k && !todos.is_empty() {
            let i = self.indice(todos.len()).unwrap();
            out.push(todos.swap_remove(i));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn min(m: u64, s: u64) -> Duration {
        Duration::from_secs(m * 60 + s)
    }

    #[test]
    fn fases_do_treino() {
        assert_eq!(fase(min(0, 0)), Fase::Normal);
        assert_eq!(fase(min(24, 59)), Fase::Normal);
        assert_eq!(fase(min(25, 0)), Fase::Aviso);
        assert_eq!(fase(min(29, 59)), Fase::Aviso);
        assert_eq!(fase(min(30, 0)), Fase::Esgotado);
        assert_eq!(fase(min(45, 0)), Fase::Esgotado);
    }

    #[test]
    fn contagem_regressiva() {
        assert_eq!(mmss(restante(LIMITE_TREINO, min(0, 0))), "30:00");
        assert_eq!(mmss(restante(LIMITE_TREINO, min(12, 34))), "17:26");
        assert_eq!(mmss(restante(LIMITE_TREINO, min(31, 0))), "00:00");
        assert_eq!(mmss(restante(TIMER_HABITO, min(9, 59))), "00:01");
        assert_eq!(restante(TIMER_HABITO, min(10, 0)), Duration::ZERO);
        assert_eq!(mmss(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn minutos_arredondados() {
        assert_eq!(minutos(min(0, 29)), 0);
        assert_eq!(minutos(min(0, 30)), 1);
        assert_eq!(minutos(min(10, 10)), 10);
        assert_eq!(minutos(min(29, 45)), 30);
    }

    #[test]
    fn sorteio_dentro_do_intervalo_e_previsivel() {
        let mut a = Sorteio::new(7);
        let mut b = Sorteio::new(7);
        for _ in 0..1000 {
            let i = a.indice(7).unwrap();
            assert!(i < 7);
            assert_eq!(Some(i), b.indice(7));
        }
        assert_eq!(a.indice(0), None);
    }

    #[test]
    fn sorteio_cobre_todas_as_opcoes() {
        let mut s = Sorteio::new(12345);
        let mut visto = [false; 8];
        for _ in 0..500 {
            visto[s.indice(8).unwrap()] = true;
        }
        assert!(visto.iter().all(|v| *v));
    }

    #[test]
    fn duas_ideias_sempre_diferentes() {
        let mut s = Sorteio::new(99);
        for _ in 0..200 {
            let v = s.distintos(5, 2);
            assert_eq!(v.len(), 2);
            assert_ne!(v[0], v[1]);
            assert!(v.iter().all(|i| *i < 5));
        }
        assert_eq!(s.distintos(1, 2), vec![0]);
        assert!(s.distintos(0, 2).is_empty());
    }
}
