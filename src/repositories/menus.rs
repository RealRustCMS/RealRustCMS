use crate::{
    error::Result,
    models::{Menu, MenuItem, MenuItemArvore, MenuItemForm, SalvarMenuNo},
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct MenusRepo<'a> {
    db: &'a PgPool,
}

impl<'a> MenusRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    // ─── MENU ─────────────────────────────────────────────

    pub async fn buscar_menu_principal(&self) -> Result<Menu> {
        let menu = sqlx::query_as!(
            Menu,
            "SELECT id, nome, slug, criado_em FROM menus WHERE slug = 'menu-principal'"
        )
        .fetch_optional(self.db)
        .await?
        .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(menu)
    }

    // ─── ITENS (lista plana) ───────────────────────────────

    // Busca todos os itens de um menu ordenados por pai_id NULLS FIRST, ordem.
    // A ordenação garante que raízes venham antes dos filhos — importante
    // para montar_arvore() funcionar corretamente com uma única passagem.
    pub async fn listar_itens(&self, menu_id: &str) -> Result<Vec<MenuItem>> {
        let itens = sqlx::query_as!(
            MenuItem,
            "SELECT id, menu_id, pai_id, rotulo, tipo, referencia_id,
                    url_externa, ordem, abrir_nova_aba, restrito
             FROM menu_itens
             WHERE menu_id = $1
             ORDER BY pai_id NULLS FIRST, ordem ASC",
            menu_id
        )
        .fetch_all(self.db)
        .await?;
        Ok(itens)
    }

    // ─── ÁRVORE ───────────────────────────────────────────

    // Ponto de entrada principal — usado pelo cache no startup e após cada save.
    pub async fn arvore_menu_principal(&self) -> Result<Vec<MenuItemArvore>> {
        let menu = self.buscar_menu_principal().await?;
        let itens = self.listar_itens(&menu.id).await?;
        let urls = self.resolver_urls(&itens).await;
        Ok(montar_arvore(itens, urls, None))
    }

    // Resolve a URL de cada item a partir do tipo + referencia_id.
    // Retorna um Vec paralelo ao Vec<MenuItem> de entrada — mesma ordem, mesma len.
    // Queries em batch por tipo para evitar N+1.
    async fn resolver_urls(&self, itens: &[MenuItem]) -> Vec<String> {
        // Coleta ids por tipo para buscar slugs em batch
        let ids_pagina: Vec<&str> = itens
            .iter()
            .filter(|i| i.tipo == "pagina")
            .filter_map(|i| i.referencia_id.as_deref())
            .collect();

        let ids_artigo: Vec<&str> = itens
            .iter()
            .filter(|i| i.tipo == "artigo")
            .filter_map(|i| i.referencia_id.as_deref())
            .collect();

        let ids_categoria: Vec<&str> = itens
            .iter()
            .filter(|i| i.tipo == "categoria")
            .filter_map(|i| i.referencia_id.as_deref())
            .collect();

        let ids_tag: Vec<&str> = itens
            .iter()
            .filter(|i| i.tipo == "tag")
            .filter_map(|i| i.referencia_id.as_deref())
            .collect();

        // Busca slugs em batch — uma query por tipo
        let slugs_pagina = buscar_slugs(self.db, "paginas", &ids_pagina).await;
        let slugs_artigo = buscar_slugs(self.db, "artigos", &ids_artigo).await;
        let slugs_categoria = buscar_slugs(self.db, "categorias", &ids_categoria).await;
        let slugs_tag = buscar_slugs(self.db, "tags", &ids_tag).await;

        // Monta Vec<String> paralelo ao Vec<MenuItem>
        itens
            .iter()
            .map(|item| {
                match item.tipo.as_str() {
                    "pagina" => match item.referencia_id.as_deref() {
                        Some(id) => {
                            let slug = slugs_pagina
                                .iter()
                                .find(|(i, _)| i == id)
                                .map(|(_, s)| s.as_str())
                                .unwrap_or("");
                            format!("/paginas/{}", slug)
                        }
                        None => "/".to_string(),
                    },
                    "artigo" => {
                        // Sem referencia_id = link para a listagem geral de artigos
                        match item.referencia_id.as_deref() {
                            Some(id) => {
                                let slug = slugs_artigo
                                    .iter()
                                    .find(|(i, _)| i == id)
                                    .map(|(_, s)| s.as_str())
                                    .unwrap_or("");
                                format!("/artigos/{}", slug)
                            }
                            None => "/artigos".to_string(),
                        }
                    }
                    "categoria" => match item.referencia_id.as_deref() {
                        Some(id) => {
                            let slug = slugs_categoria
                                .iter()
                                .find(|(i, _)| i == id)
                                .map(|(_, s)| s.as_str())
                                .unwrap_or("");
                            format!("/categoria/{}", slug)
                        }
                        None => "/artigos".to_string(),
                    },
                    "tag" => match item.referencia_id.as_deref() {
                        Some(id) => {
                            let slug = slugs_tag
                                .iter()
                                .find(|(i, _)| i == id)
                                .map(|(_, s)| s.as_str())
                                .unwrap_or("");
                            format!("/tag/{}", slug)
                        }
                        None => "/artigos".to_string(),
                    },
                    "galeria" => "/galeria".to_string(),
                    "eventos" => "/eventos".to_string(),
                    "externo" => item.url_externa.clone().unwrap_or_default(),
                    _ => String::new(),
                }
            })
            .collect()
    }

    // ─── CRUD DE ITENS ────────────────────────────────────

    pub async fn adicionar_item(&self, menu_id: &str, dados: &MenuItemForm) -> Result<String> {
        let id = Uuid::new_v4().to_string();

        // Próxima ordem = max(ordem) dos itens raiz + 1
        let proxima_ordem = sqlx::query_scalar!(
            "SELECT COALESCE(MAX(ordem), -1) FROM menu_itens
             WHERE menu_id = $1 AND pai_id IS NULL",
            menu_id
        )
        .fetch_one(self.db)
        .await?
        .unwrap_or(-1)
            + 1;

        let abrir_nova_aba = dados.abrir_nova_aba.is_some();
        let restrito = dados.restrito.is_some();

        // Rótulo padrão quando o campo vem vazio — tenta buscar o título real
        // do recurso referenciado; cai em literal genérico apenas se não achar.
        let rotulo = if dados.rotulo.trim().is_empty() {
            match dados.tipo.as_str() {
                "pagina" => {
                    let titulo = if let Some(ref_id) = dados.referencia_id.as_deref() {
                        sqlx::query_scalar!("SELECT titulo FROM paginas WHERE id = $1", ref_id)
                            .fetch_optional(self.db)
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    titulo.unwrap_or_else(|| "Página".to_string())
                }
                "artigo" => {
                    let titulo = if let Some(ref_id) = dados.referencia_id.as_deref() {
                        sqlx::query_scalar!("SELECT titulo FROM artigos WHERE id = $1", ref_id)
                            .fetch_optional(self.db)
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    titulo.unwrap_or_else(|| "Artigos".to_string())
                }
                "categoria" => {
                    let nome = if let Some(ref_id) = dados.referencia_id.as_deref() {
                        sqlx::query_scalar!("SELECT nome FROM categorias WHERE id = $1", ref_id)
                            .fetch_optional(self.db)
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    nome.unwrap_or_else(|| "Categoria".to_string())
                }
                "tag" => {
                    let nome = if let Some(ref_id) = dados.referencia_id.as_deref() {
                        sqlx::query_scalar!("SELECT nome FROM tags WHERE id = $1", ref_id)
                            .fetch_optional(self.db)
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    nome.unwrap_or_else(|| "Tag".to_string())
                }
                "galeria" => "Galeria".to_string(),
                "eventos" => "Eventos".to_string(),
                _ => "Item".to_string(),
            }
        } else {
            dados.rotulo.trim().to_string()
        };

        sqlx::query!(
            "INSERT INTO menu_itens
             (id, menu_id, pai_id, rotulo, tipo, referencia_id, url_externa, ordem, abrir_nova_aba, restrito)
             VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, $8, $9)",
            id,
            menu_id,
            rotulo,
            dados.tipo,
            dados.referencia_id,
            dados.url_externa,
            proxima_ordem,
            abrir_nova_aba,
            restrito
        )
        .execute(self.db)
        .await?;

        Ok(id)
    }

    pub async fn deletar_item(&self, id: &str) -> Result<()> {
        // ON DELETE CASCADE na migration cuida dos filhos automaticamente
        sqlx::query!("DELETE FROM menu_itens WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    // ─── SALVAR ÁRVORE (drag-and-drop) ────────────────────

    // Recebe a árvore serializada pelo editor JS e persiste no banco.
    // Estratégia: percorre a árvore recursivamente e faz UPDATE de
    // pai_id + ordem para cada item — sem deletar/recriar, preservando ids.
    pub async fn salvar_arvore(&self, menu_id: &str, nos: &[SalvarMenuNo]) -> Result<()> {
        salvar_nos_recursivo(self.db, menu_id, nos, None, 0).await
    }
}

// ─── HELPERS PRIVADOS ─────────────────────────────────────

// Monta a árvore recursiva a partir da lista plana retornada pelo banco.
//
// Algoritmo: filtra os filhos diretos do pai_id atual, para cada um
// chama recursivamente com o id desse filho como novo pai_id.
// O caso base é quando não há mais filhos — retorna Vec vazio implicitamente.
//
// urls é um Vec paralelo a itens — mesma posição = mesma URL resolvida.
// Consumimos por índice para evitar clone desnecessário.
fn montar_arvore(
    itens: Vec<MenuItem>,
    urls: Vec<String>,
    pai_id: Option<&str>,
) -> Vec<MenuItemArvore> {
    // Coleta índices dos filhos diretos deste pai
    let indices_filhos: Vec<usize> = itens
        .iter()
        .enumerate()
        .filter(|(_, item)| item.pai_id.as_deref() == pai_id)
        .map(|(i, _)| i)
        .collect();

    indices_filhos
        .into_iter()
        .map(|i| {
            let item = &itens[i];
            let url = urls[i].clone();
            let id = item.id.clone();

            // Recursão: busca filhos deste item pelo seu id
            let filhos = montar_arvore_ref(&itens, &urls, Some(id.as_str()));

            MenuItemArvore {
                id,
                rotulo: item.rotulo.clone(),
                url,
                abrir_nova_aba: item.abrir_nova_aba,
                restrito: item.restrito,
                filhos,
            }
        })
        .collect()
}

// Versão por referência para a recursão interna — evita mover o Vec
fn montar_arvore_ref(
    itens: &[MenuItem],
    urls: &[String],
    pai_id: Option<&str>,
) -> Vec<MenuItemArvore> {
    itens
        .iter()
        .enumerate()
        .filter(|(_, item)| item.pai_id.as_deref() == pai_id)
        .map(|(i, item)| {
            let filhos = montar_arvore_ref(itens, urls, Some(item.id.as_str()));
            MenuItemArvore {
                id: item.id.clone(),
                rotulo: item.rotulo.clone(),
                url: urls[i].clone(),
                abrir_nova_aba: item.abrir_nova_aba,
                restrito: item.restrito,
                filhos,
            }
        })
        .collect()
}

// Busca pares (id, slug) de uma tabela em batch.
// Retorna Vec vazio se a query falhar — degradação silenciosa no menu.
async fn buscar_slugs(db: &PgPool, tabela: &str, ids: &[&str]) -> Vec<(String, String)> {
    if ids.is_empty() {
        return vec![];
    }
    // ANY($1) com array de texto — padrão SQLx para IN com slice
    let query = format!("SELECT id, slug FROM {} WHERE id = ANY($1)", tabela);
    sqlx::query_as::<_, (String, String)>(&query)
        .bind(ids)
        .fetch_all(db)
        .await
        .unwrap_or_default()
}

// Persiste a árvore recursivamente atualizando pai_id e ordem de cada nó.
// profundidade é passada apenas para logging futuro — não usada no SQL.
async fn salvar_nos_recursivo(
    db: &PgPool,
    menu_id: &str,
    nos: &[SalvarMenuNo],
    pai_id: Option<&str>,
    _profundidade: u32,
) -> Result<()> {
    for (ordem, no) in nos.iter().enumerate() {
        sqlx::query!(
            "UPDATE menu_itens
             SET pai_id = $1, ordem = $2
             WHERE id = $3 AND menu_id = $4",
            pai_id,
            ordem as i32,
            no.id,
            menu_id
        )
        .execute(db)
        .await?;

        // Recursão para os filhos deste nó
        if !no.filhos.is_empty() {
            Box::pin(salvar_nos_recursivo(
                db,
                menu_id,
                &no.filhos,
                Some(&no.id),
                _profundidade + 1,
            ))
            .await?;
        }
    }
    Ok(())
}