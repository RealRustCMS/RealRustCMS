use axum::http::HeaderMap;
use std::net::SocketAddr;

// Resolve o IP real do visitante quando o app roda atrás do Caddy (ver
// deploy/nova-instancia.sh — reverse_proxy 127.0.0.1:PORTA). Sem isso,
// ConnectInfo sempre reporta o IP do proxy (127.0.0.1), colapsando rate
// limiting e o UNIQUE(artigo_id, ip) de avaliações num único "visitante"
// coletivo — todo mundo atrás do Caddy vira o mesmo IP.
//
// Caddy sempre ANEXA o IP que ele mesmo observou ao final de
// X-Forwarded-For (nunca sobrescreve o que o cliente mandou) — por isso
// usamos o ÚLTIMO valor da lista, não o primeiro: qualquer coisa antes dele
// pode ter sido forjada pelo próprio cliente. Assume que o Caddy é a única
// borda voltada pra internet (topologia deste deploy/); se um dia entrar
// outro proxy/CDN na frente do Caddy, essa premissa muda.
pub fn do_cliente(headers: &HeaderMap, conectado: SocketAddr) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| conectado.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conectado() -> SocketAddr {
        "127.0.0.1:8080".parse().unwrap()
    }

    #[test]
    fn sem_header_usa_connectinfo() {
        let headers = HeaderMap::new();
        assert_eq!(do_cliente(&headers, conectado()), "127.0.0.1");
    }

    #[test]
    fn usa_ultimo_valor_da_lista_nao_o_primeiro() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "8.8.8.8, 203.0.113.7".parse().unwrap());
        // 203.0.113.7 é o que o Caddy observou de verdade; 8.8.8.8 pode
        // ter sido forjado pelo cliente mandando o header ele mesmo.
        assert_eq!(do_cliente(&headers, conectado()), "203.0.113.7");
    }

    #[test]
    fn header_vazio_cai_pro_connectinfo() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "".parse().unwrap());
        assert_eq!(do_cliente(&headers, conectado()), "127.0.0.1");
    }
}
