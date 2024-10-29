use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordVerifier;
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use surrealdb::Surreal;
use argon2::PasswordHasher;

use surrealdb::engine::local::RocksDb;

use crate::Context;

pub async fn init_db() -> surrealdb::Result<Surreal<surrealdb::engine::local::Db>> {
    let db = Surreal::new::<RocksDb>("./database").await?;

    db.use_ns("void").use_db("credentials").await?;

    Ok(db)
}

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    name: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct Record {
    #[allow(dead_code)]
    id: RecordId,
}

impl Context {
    pub async fn player_exists(&self, name: &str) -> anyhow::Result<bool> {
        let users: Vec<Credentials> = self.db.select("credentials").await?;
        let user = users.iter().find(|a| a.name == name);
        Ok(user.is_some())
    }

    pub async fn register(&self, name: &str, password: &str) -> anyhow::Result<bool> {
        if self.player_exists(&name).await? {
            return Ok(false);
        }

        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        let hash = argon2.hash_password(password.as_bytes(), &salt)?;
        let hash = hash.serialize().to_string();

        let _: Option<Record> = self
            .db
            .create("credentials")
            .content(Credentials {
                name: name.to_string(),
                hash,
            })
            .await?;

        Ok(true)
    }

    pub async fn authenticate(&self, name: &str, password: &str) -> anyhow::Result<bool> {
        if !self.player_exists(&name).await? {
            return Ok(false);
        }

        let argon2 = Argon2::default();

        let users: Vec<Credentials> = self.db.select("credentials").await?;
        let user = users.iter().find(|a| a.name == name);

        if let Some(user) = user {
            let hash = PasswordHash::new(&user.hash)?;

            if argon2.verify_password(password.as_bytes(), &hash).is_ok() {
                return Ok(true);
            }
        }

        Ok(false)
    }
}