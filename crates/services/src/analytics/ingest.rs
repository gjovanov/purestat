use clickhouse::Client;
use purestat_db::clickhouse::schemas::Event;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Buffer full")]
    BufferFull,
}

/// How many events wait in memory at most while ClickHouse is unreachable.
/// At roughly 1 KB an event this is about 20 MB. Past it the OLDEST events
/// are dropped, and every drop is counted and logged.
pub const DEFAULT_MAX_BUFFERED: usize = 20_000;

/// After a failed flush, new events stop triggering flushes for this long.
/// The flush timer keeps retrying on its own interval, so an outage costs one
/// attempt per tick instead of one per incoming event.
const RETRY_BACKOFF: Duration = Duration::from_secs(5);

pub struct IngestService {
    client: Client,
    buffer: Arc<Mutex<VecDeque<Event>>>,
    batch_size: usize,
    max_buffered: usize,
    /// When the last flush failed; `None` once one succeeds.
    last_failure: Mutex<Option<Instant>>,
    /// Set while a flush is writing: one batch in flight at a time, so
    /// requests never pile up behind a slow or stalled ClickHouse.
    flushing: AtomicBool,
    /// Events dropped to the cap since the last report.
    dropped: AtomicU64,
}

impl IngestService {
    pub fn new(client: Client, batch_size: usize) -> Self {
        Self::with_max_buffered(client, batch_size, DEFAULT_MAX_BUFFERED)
    }

    pub fn with_max_buffered(client: Client, batch_size: usize, max_buffered: usize) -> Self {
        Self {
            client,
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(batch_size))),
            batch_size,
            max_buffered: max_buffered.max(batch_size),
            last_failure: Mutex::new(None),
            flushing: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
        }
    }

    /// Buffers the event; it is never lost to a failed flush. A full batch
    /// triggers a flush unless one failed within RETRY_BACKOFF or one is
    /// already in flight. A failure here is logged and left to the timer.
    pub async fn ingest(&self, event: Event) -> Result<(), IngestError> {
        let should_flush;
        {
            let mut buf = self.buffer.lock().await;
            buf.push_back(event);
            if buf.len() > self.max_buffered {
                buf.pop_front();
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            should_flush = buf.len() >= self.batch_size;
        }
        if should_flush
            && !self.backing_off().await
            && let Err(e) = self.flush().await
        {
            warn!(error = %e, "Flush failed; the events stay buffered for the next attempt");
        }
        Ok(())
    }

    /// Writes the buffered events to ClickHouse. On failure the batch goes
    /// back to the FRONT of the buffer (it is older than anything that arrived
    /// meanwhile), trimmed to the cap, and the error is returned.
    pub async fn flush(&self) -> Result<(), IngestError> {
        if self
            .flushing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(()); // a flush is already in flight; it takes these events too, or the timer will
        }
        let result = self.flush_once().await;
        self.flushing.store(false, Ordering::Release);
        result
    }

    async fn flush_once(&self) -> Result<(), IngestError> {
        let events = {
            let mut buf = self.buffer.lock().await;
            if buf.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *buf)
        };

        let count = events.len();
        match self.write(&events).await {
            Ok(()) => {
                *self.last_failure.lock().await = None;
                info!(count = count, "Flushed events to ClickHouse");
                self.report_drops();
                Ok(())
            }
            Err(e) => {
                *self.last_failure.lock().await = Some(Instant::now());
                {
                    let mut buf = self.buffer.lock().await;
                    let newer = std::mem::take(&mut *buf);
                    *buf = events;
                    buf.extend(newer);
                    while buf.len() > self.max_buffered {
                        buf.pop_front();
                        self.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
                self.report_drops();
                Err(e)
            }
        }
    }

    async fn write(&self, events: &VecDeque<Event>) -> Result<(), IngestError> {
        let mut inserter = self.client.insert("events")?;
        for event in events {
            inserter.write(event).await?;
        }
        inserter.end().await?;
        Ok(())
    }

    async fn backing_off(&self) -> bool {
        matches!(*self.last_failure.lock().await, Some(at) if at.elapsed() < RETRY_BACKOFF)
    }

    fn report_drops(&self) {
        let dropped = self.dropped.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            error!(
                dropped = dropped,
                max_buffered = self.max_buffered,
                "Event buffer full while ClickHouse was unreachable: dropped the oldest events"
            );
        }
    }

    /// Events waiting to be written.
    pub async fn buffered(&self) -> usize {
        self.buffer.lock().await.len()
    }

    pub fn start_flush_timer(self: &Arc<Self>, interval_ms: u64) {
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                if let Err(e) = svc.flush().await {
                    let buffered = svc.buffered().await;
                    error!(error = %e, buffered = buffered, "Failed to flush events; kept them for the next tick");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client whose every request fails at once: nothing listens on port 9.
    fn unreachable() -> Client {
        Client::default().with_url("http://127.0.0.1:9")
    }

    fn event(path: &str) -> Event {
        Event {
            site_id: 7,
            visitor_hash: "v".into(),
            session_id: "s".into(),
            event_name: "pageview".into(),
            url: format!("https://example.com{path}"),
            path: path.into(),
            hostname: "example.com".into(),
            referrer: String::new(),
            referrer_source: String::new(),
            utm_source: String::new(),
            utm_medium: String::new(),
            utm_campaign: String::new(),
            utm_content: String::new(),
            utm_term: String::new(),
            country: String::new(),
            region: String::new(),
            city: String::new(),
            browser: String::new(),
            browser_version: String::new(),
            os: String::new(),
            os_version: String::new(),
            device_type: String::new(),
            screen_width: 0,
            screen_height: 0,
            prop_keys: vec![],
            prop_values: vec![],
            revenue_amount: None,
            revenue_currency: None,
            timestamp: time::OffsetDateTime::now_utc(),
        }
    }

    async fn paths(svc: &IngestService) -> Vec<String> {
        svc.buffer
            .lock()
            .await
            .iter()
            .map(|e| e.path.clone())
            .collect()
    }

    #[tokio::test]
    async fn a_failed_flush_keeps_its_events_in_order() {
        let svc = IngestService::new(unreachable(), 1_000);
        for p in ["/a", "/b", "/c"] {
            svc.ingest(event(p)).await.unwrap();
        }
        assert!(svc.flush().await.is_err(), "ClickHouse is unreachable");
        assert_eq!(
            paths(&svc).await,
            ["/a", "/b", "/c"],
            "the batch is back, oldest first"
        );
    }

    #[tokio::test]
    async fn events_that_arrive_during_a_failure_queue_behind_the_failed_batch() {
        let svc = IngestService::new(unreachable(), 1_000);
        svc.ingest(event("/old")).await.unwrap();
        assert!(svc.flush().await.is_err());
        svc.ingest(event("/new")).await.unwrap();
        assert!(svc.flush().await.is_err());
        assert_eq!(paths(&svc).await, ["/old", "/new"]);
    }

    #[tokio::test]
    async fn ingest_succeeds_even_when_its_flush_fails() {
        // batch_size 1: every event triggers a flush, which fails.
        let svc = IngestService::new(unreachable(), 1);
        assert!(
            svc.ingest(event("/a")).await.is_ok(),
            "the event is buffered, not rejected"
        );
        assert_eq!(svc.buffered().await, 1);
    }

    #[tokio::test]
    async fn the_buffer_is_capped_by_dropping_the_oldest() {
        let svc = IngestService::with_max_buffered(unreachable(), 1_000, 1_000);
        for i in 0..1_003 {
            svc.ingest(event(&format!("/p{i}"))).await.unwrap();
        }
        let kept = paths(&svc).await;
        assert_eq!(kept.len(), 1_000);
        assert_eq!(
            kept.first().map(String::as_str),
            Some("/p3"),
            "the three oldest went"
        );
        assert_eq!(svc.dropped.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn after_a_failure_new_events_do_not_retry_until_the_backoff_passes() {
        let svc = IngestService::new(unreachable(), 1);
        svc.ingest(event("/a")).await.unwrap(); // flush attempt 1 fails
        let failed_at = svc
            .last_failure
            .lock()
            .await
            .expect("the failure is recorded");
        svc.ingest(event("/b")).await.unwrap(); // inside the backoff: no attempt
        assert_eq!(
            *svc.last_failure.lock().await,
            Some(failed_at),
            "no second attempt was made inside the backoff"
        );
        assert_eq!(paths(&svc).await, ["/a", "/b"]);
    }
}
