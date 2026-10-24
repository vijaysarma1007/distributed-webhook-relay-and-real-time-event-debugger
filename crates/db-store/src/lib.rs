use anyhow::{Context, Ok, Result};
use core_types::{DeliveryStatus, IngestWebhookRequest, OutBoxEvent};
use sqlx::{self, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct DbStore {
    pool: PgPool,
}

impl DbStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn insert_outbox_events(&self, req: IngestWebhookRequest) -> Result<Uuid> {
        let event_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO outbox_events (id, destination_url, payload, secret_key)
             VALUES 
             ($1, $2, $3, $4)
             "#,
            event_id,
            req.destination_url,
            req.payload,
            req.secret_key
        )
        .execute(&self.pool)
        .await
        .context("Inserting into outbox events")?;

        Ok(event_id)
    }

    pub async fn fetch_pending_events(&self, batch_size: i64) -> Result<Vec<OutBoxEvent>> {
        let status: String = DeliveryStatus::Pending.into();
        sqlx::query_as!(OutBoxEvent, r#"SELECT id, destination_url, payload, secret_key, status, attempt_count, max_attempts, next_retry_at, created_at
         FROM outbox_events 
         WHERE status = $1 AND next_retry_at <= NOW() 
         ORDER BY next_retry_at ASC 
         LIMIT $2 
         FOR UPDATE SKIP LOCKED
         "#, status, batch_size
        ).fetch_all(&self.pool).await.context("fetching outbox box event with status pending")
    }
}
