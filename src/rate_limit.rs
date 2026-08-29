use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::time::{Duration, Instant};

// ─── Configurações por tipo de ação ──────────────────────────────────────────
//
// LOGIN: poucos tentativas, bloqueio longo — protege contra força bruta.
// AVALIACAO: mais tentativas, sem bloqueio — evita spam mas não trava usuário legítimo.
//            A UNIQUE constraint no banco já garante 1 voto por IP por artigo;
//            o rate limit aqui evita flood de requisições.
// COMENTARIO: moderado — o Turnstile já filtra bots quando habilitado.

struct Politica {
    max_tentativas: u32,
    janela: Duration,
    bloqueio: Option<Duration>, // None = não bloqueia, só rejeita na janela
}

const POLITICA_LOGIN: Politica = Politica {
    max_tentativas: 5,
    janela: Duration::from_secs(300),         // 5 minutos
    bloqueio: Some(Duration::from_secs(900)), // 15 minutos
};

const POLITICA_AVALIACAO: Politica = Politica {
    max_tentativas: 10,
    janela: Duration::from_secs(60), // 10 avaliações por minuto por IP
    bloqueio: None,                  // sem bloqueio persistente
};

const POLITICA_COMENTARIO: Politica = Politica {
    max_tentativas: 5,
    janela: Duration::from_secs(300), // 5 comentários por 5 minutos por IP
    bloqueio: None,
};

// ─── Chave composta: "acao:ip" ────────────────────────────────────────────────
// Separar por ação evita que tentativas de login interfiram no contador
// de avaliações do mesmo IP, e vice-versa.

struct Tentativas {
    count: u32,
    primeira: Instant,
    bloqueado_ate: Option<Instant>,
}

static TENTATIVAS: Lazy<DashMap<String, Tentativas>> = Lazy::new(DashMap::new);

fn verificar(chave: &str, politica: &Politica) -> Result<(), String> {
    let agora = Instant::now();

    let mut entry = TENTATIVAS.entry(chave.to_string()).or_insert(Tentativas {
        count: 0,
        primeira: agora,
        bloqueado_ate: None,
    });

    // Verifica bloqueio ativo
    if let Some(bloqueado_ate) = entry.bloqueado_ate {
        if agora < bloqueado_ate {
            let restante = bloqueado_ate.duration_since(agora).as_secs();
            return Err(format!(
                "Muitas tentativas. Tente novamente em {} minutos.",
                restante / 60 + 1
            ));
        } else {
            entry.count = 0;
            entry.primeira = agora;
            entry.bloqueado_ate = None;
        }
    }

    // Janela expirou — reseta
    if agora.duration_since(entry.primeira) > politica.janela {
        entry.count = 0;
        entry.primeira = agora;
    }

    entry.count += 1;

    if entry.count > politica.max_tentativas {
        if let Some(duracao_bloqueio) = politica.bloqueio {
            entry.bloqueado_ate = Some(agora + duracao_bloqueio);
            return Err(format!(
                "Muitas tentativas. Tente novamente em {} minutos.",
                duracao_bloqueio.as_secs() / 60
            ));
        } else {
            return Err("Muitas tentativas. Aguarde um momento.".into());
        }
    }

    Ok(())
}

// ─── Sweep periódico ─────────────────────────────────────────────────────────
//
// DashMap acumula entradas para cada IP único visto. O sweep remove entradas
// que não têm nem bloqueio ativo nem janela vigente. Roda a cada 5 min.
// A janela mais longa de qualquer política é 900s (bloqueio de login), então
// qualquer entrada mais antiga que isso sem bloqueio ativo é morta.

pub fn iniciar_sweep_periodico() {
    tokio::spawn(async {
        let mut intervalo = tokio::time::interval(Duration::from_secs(300));
        loop {
            intervalo.tick().await;
            let agora = Instant::now();
            TENTATIVAS.retain(|_, v| {
                if let Some(bloqueado_ate) = v.bloqueado_ate {
                    if agora < bloqueado_ate {
                        return true; // bloqueio ainda vigente
                    }
                }
                agora.duration_since(v.primeira) <= Duration::from_secs(900)
            });
        }
    });
}

// ─── API pública ─────────────────────────────────────────────────────────────

pub fn verificar_ip(ip: &str) -> Result<(), String> {
    verificar(&format!("login:{}", ip), &POLITICA_LOGIN)
}

pub fn verificar_avaliacao(ip: &str) -> Result<(), String> {
    verificar(&format!("avaliacao:{}", ip), &POLITICA_AVALIACAO)
}

pub fn verificar_comentario(ip: &str) -> Result<(), String> {
    verificar(&format!("comentario:{}", ip), &POLITICA_COMENTARIO)
}

pub fn registrar_sucesso(ip: &str) {
    // Remove só o contador de login — os outros expiram naturalmente na janela
    TENTATIVAS.remove(&format!("login:{}", ip));
}
