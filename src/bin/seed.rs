#[tokio::main]
async fn main() {
    let env_path = std::env::var("CARGO_MANIFEST_DIR")
        .map(|dir| std::path::PathBuf::from(dir).join(".env"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".env"));
    dotenvy::from_path(&env_path).ok();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!("Uso: cargo run --bin seed -- <nome> <email> <senha>");
        eprintln!("Exemplo: cargo run --bin seed -- \"João Silva\" joao@email.com minhasenha123");
        std::process::exit(1);
    }

    let nome = &args[1];
    let email = &args[2];
    let senha = &args[3];

    if senha.len() < 8 {
        eprintln!("✗ A senha deve ter pelo menos 8 caracteres.");
        std::process::exit(1);
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL não definida");
    let db = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Falha ao conectar no banco");

    let token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    // query_scalar! no Postgres retorna Option<i64> para COUNT(*)
    let existe = sqlx::query_scalar!("SELECT COUNT(*) FROM usuarios WHERE email ILIKE $1", email)
        .fetch_one(&db)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    if existe == 0 {
        let service = rustcms::services::auth::AuthService::novo(&db);
        match service
            .criar_usuario(nome, email, Some(senha), "admin")
            .await
        {
            Ok(u) => println!("✓ Usuário criado: {} ({})", u.nome, u.email),
            Err(e) => {
                eprintln!("✗ Erro ao criar usuário: {:?}", e);
                return;
            }
        }
    } else {
        let hash = rustcms::services::auth::hash_senha(senha)
            .expect("Falha ao gerar hash da senha");
        sqlx::query!(
            "UPDATE usuarios SET senha_hash = $1 WHERE email ILIKE $2",
            hash,
            email
        )
        .execute(&db)
        .await
        .expect("Falha ao redefinir senha");
        println!("✓ Usuário {} já existe — senha redefinida", email);
    }

    sqlx::query!(
        "UPDATE usuarios SET api_token = $1 WHERE email ILIKE $2",
        token,
        email
    )
    .execute(&db)
    .await
    .expect("Falha ao atualizar token");

    println!("✓ Token gerado: {}", token);
    println!("  Use no header: Authorization: Bearer {}", token);
}
