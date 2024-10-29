use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use surrealdb::Surreal;

use surrealdb::engine::local::RocksDb;

pub async fn init_db() -> surrealdb::Result<Surreal<surrealdb::engine::local::Db>> {
    let db = Surreal::new::<RocksDb>("./database").await?;

    db.use_ns("void").use_db("credentials").await?;

    Ok(db)
}