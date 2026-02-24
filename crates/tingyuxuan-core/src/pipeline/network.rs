use std::time::Duration;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::pipeline::events::PipelineEvent;

/// Periodically checks network connectivity by issuing an HTTP HEAD request
/// and emits [`PipelineEvent::NetworkStatusChanged`] whenever the reachability
/// state changes.
pub struct NetworkMonitor {
    check_url: String,
    interval: Duration,
    timeout: Duration,
}

impl NetworkMonitor {
    /// Create a new monitor that will probe the given URL.
    ///
    /// Defaults: check every 30 s, 5 s HTTP timeout.
    pub fn new(check_url: String) -> Self {
        Self {
            check_url,
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
        }
    }

    /// Spawn a background Tokio task that periodically probes `check_url`
    /// with an HTTP HEAD request.  An event is emitted **only** when the
    /// online/offline state actually changes.
    ///
    /// Returns a [`CancellationToken`] — drop it or call `.cancel()` to
    /// stop the background task.
    pub fn start(&self, event_tx: broadcast::Sender<PipelineEvent>) -> CancellationToken {
        let token = CancellationToken::new();
        let child = token.child_token();

        let url = self.check_url.clone();
        let interval = self.interval;
        let timeout = self.timeout;

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("failed to build reqwest client");

            let mut was_online: Option<bool> = None;

            loop {
                let online = client.head(&url).send().await.is_ok();

                let changed = was_online != Some(online);
                if changed {
                    debug!(online, "network status changed");
                    let _ = event_tx.send(PipelineEvent::NetworkStatusChanged { online });
                    was_online = Some(online);
                }

                tokio::select! {
                    _ = tokio::time::sleep(interval) => {}
                    _ = child.cancelled() => {
                        debug!("network monitor cancelled");
                        break;
                    }
                }
            }
        });

        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

    /// Helper: build a monitor with short intervals so tests finish quickly.
    fn fast_monitor(url: String) -> NetworkMonitor {
        NetworkMonitor {
            check_url: url,
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(200),
        }
    }

    #[tokio::test]
    async fn emits_online_when_server_is_up() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let monitor = fast_monitor(server.uri());
        let (tx, mut rx) = broadcast::channel(16);

        let token = monitor.start(tx);

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel error");

        match event {
            PipelineEvent::NetworkStatusChanged { online } => assert!(online),
            other => panic!("unexpected event: {:?}", other),
        }

        token.cancel();
    }

    #[tokio::test]
    async fn emits_offline_when_server_is_down() {
        // Point at a URL that will definitely refuse connections.
        let monitor = fast_monitor("http://127.0.0.1:1".to_string());
        let (tx, mut rx) = broadcast::channel(16);

        let token = monitor.start(tx);

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel error");

        match event {
            PipelineEvent::NetworkStatusChanged { online } => assert!(!online),
            other => panic!("unexpected event: {:?}", other),
        }

        token.cancel();
    }

    #[tokio::test]
    async fn does_not_emit_duplicate_events() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200))
            .expect(2..)
            .mount(&server)
            .await;

        let monitor = fast_monitor(server.uri());
        let (tx, mut rx) = broadcast::channel(16);

        let token = monitor.start(tx);

        // First event — online.
        let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out")
            .expect("channel error");
        assert!(matches!(
            first,
            PipelineEvent::NetworkStatusChanged { online: true }
        ));

        // Wait long enough for several ticks — should NOT get another event
        // because the status has not changed.
        let second = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;
        assert!(second.is_err(), "should not receive a duplicate event");

        token.cancel();
    }
}
