//! The `serve` command, run as the real binary: it migrates, answers
//! `/health`, and sends a login link through the outbox.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::process::{Child, Command, Stdio};

use sqlx::PgPool;

async fn database_url(pool: &PgPool) -> String {
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").unwrap();
    format!("{}/{name}", base.rsplit_once('/').unwrap().0)
}

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[sqlx::test(migrations = false)]
async fn serve_starts_and_answers_health(pool: PgPool) {
    let port = 39_000 + u16::try_from(std::process::id() % 1000).unwrap();
    let _server = Killed(
        Command::new(env!("CARGO_BIN_EXE_lab-report-assistant"))
            .arg("serve")
            .env("DATABASE_URL", database_url(&pool).await)
            .env("BIND_ADDR", format!("127.0.0.1:{port}"))
            .env("MAILPIT_URL", "http://127.0.0.1:1")
            .env_remove("OWNER_ANTHROPIC_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut answer = None;
    for _ in 0..100 {
        if let Ok(mut stream) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            let mut text = String::new();
            stream.read_to_string(&mut text).await.unwrap();
            answer = Some(text);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let answer = answer.expect("the server never listened");
    assert!(answer.starts_with("HTTP/1.1 200") && answer.ends_with("ok"), "{answer}");
}
