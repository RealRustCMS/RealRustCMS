// Sanitização de HTML gerado pelo editor Quill antes de persistir no banco.
// A allowlist padrão do ammonia (h1-h6, strong, em, u, s, ol/ul/li, blockquote,
// pre/code, a[href], img[src,alt,width,height]) já cobre exatamente as tags
// que Quill.getSemanticHTML() produz com a toolbar configurada em
// artigo_form.html e pagina_form.html — não precisa de customização.
// Protege contra XSS persistente caso uma conta de editor seja comprometida.
pub fn sanitizar_html(html: &str) -> String {
    ammonia::clean(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_script_e_handlers_inline() {
        let entrada = r#"<p onclick="alert(1)">oi</p><script>alert(1)</script><img src=x onerror=alert(1)>"#;
        let saida = sanitizar_html(entrada);
        assert!(!saida.contains("<script"));
        assert!(!saida.contains("onclick"));
        assert!(!saida.contains("onerror"));
    }

    #[test]
    fn preserva_formatacao_do_quill() {
        let entrada = r#"<h1>Título</h1><p><strong>negrito</strong> <em>itálico</em> <u>sublinhado</u></p><ol><li>um</li></ol><blockquote>cita</blockquote><a href="https://exemplo.com">link</a><img src="/uploads/foto.jpg" alt="foto">"#;
        let saida = sanitizar_html(entrada);
        assert!(saida.contains("<h1>"));
        assert!(saida.contains("<strong>"));
        assert!(saida.contains("<em>"));
        assert!(saida.contains("<u>"));
        assert!(saida.contains("<ol>"));
        assert!(saida.contains("<blockquote>"));
        assert!(saida.contains(r#"href="https://exemplo.com""#));
        assert!(saida.contains(r#"src="/uploads/foto.jpg""#));
    }
}
