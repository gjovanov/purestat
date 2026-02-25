use clickhouse::Client;
use purestat_db::clickhouse::schemas::Event;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Buffer full")]
    BufferFull,
}

pub struct IngestService {
    client: Client,
    buffer: Arc<Mutex<Vec<Event>>>,
    batch_size: usize,
}

impl IngestService {
    pub fn new(client: Client, batch_size: usize) -> Self {
        Self {
            client,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(batch_size))),
            batch_size,
        }
    }

    pub async fn ingest(&self, event: Event) -> Result<(), IngestError> {
        let should_flush;
        {
            let mut buf = self.buffer.lock().await;
            buf.push(event);
            should_flush = buf.len() >= self.batch_size;
        }
        if should_flush {
            self.flush().await?;
        }
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), IngestError> {
        let events;
        {
            let mut buf = self.buffer.lock().await;
            if buf.is_empty() {
                return Ok(());
            }
            events = std::mem::take(&mut *buf);
        }

        let count = events.len();
        let mut inserter = self.client.insert("events")?;
        for event in events {
            inserter.write(&event).await?;
        }
        inserter.end().await?;

        info!(count = count, "Flushed events to ClickHouse");
        Ok(())
    }

    pub fn start_flush_timer(self: &Arc<Self>, interval_ms: u64) {
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                if let Err(e) = svc.flush().await {
                    error!(error = %e, "Failed to flush events");
                }
            }
        });
    }
}
