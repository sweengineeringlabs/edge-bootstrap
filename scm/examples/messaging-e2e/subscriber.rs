//! Background subscriber — the one thing this module owns is proving genuine
//! round-trip pub/sub: a real `subscribe()` call against the same broker the
//! handler publishes through, counting and logging every message it actually
//! receives back. Publishing without erroring only proves the call didn't
//! fail; this module is the other half of the proof.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use swe_edge_bootstrap::MessageBroker;

use crate::broker::TOPIC;

/// Spawn a task that subscribes to [`TOPIC`] on `broker` and counts every
/// message it receives.
///
/// Returns `(counter, ready)`: `counter` is incremented once per message
/// received (poll it from a test or from `main` to prove delivery happened);
/// `ready` resolves once `subscribe()` has actually completed, so callers can
/// wait on it before publishing — per [`MessageBroker::subscribe`]'s
/// contract, only messages published *after* subscription is established are
/// ever delivered.
pub(crate) fn spawn_subscriber(
    broker: Arc<dyn MessageBroker>,
) -> (Arc<AtomicU64>, tokio::sync::oneshot::Receiver<()>) {
    let received = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&received);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let mut stream = match broker.subscribe(TOPIC).await {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("messaging: subscribe to '{TOPIC}' failed: {err}");
                return;
            }
        };
        // Signal readiness only after the subscription is actually
        // established, not merely after the task was scheduled.
        let _ = ready_tx.send(());

        while let Some(next) = stream.next().await {
            match next {
                Ok(msg) => {
                    let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    let body = String::from_utf8_lossy(&msg.payload);
                    println!("messaging: subscriber received #{count} on '{TOPIC}': {body}");
                }
                Err(err) => {
                    eprintln!("messaging: stream error on '{TOPIC}': {err}");
                }
            }
        }
    });

    (received, ready_rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap::{Message, MessageBrokerFactory};

    /// Bounds how long a test waits for the background task to observe a
    /// published message before concluding delivery genuinely failed.
    const POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    async fn wait_for_count(counter: &AtomicU64, expected: u64) {
        tokio::time::timeout(POLL_TIMEOUT, async {
            while counter.load(Ordering::SeqCst) < expected {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for counter to reach {expected}, stuck at {}",
                counter.load(Ordering::SeqCst)
            )
        });
    }

    #[tokio::test]
    async fn test_spawn_subscriber_counts_message_published_after_ready() {
        let broker: Arc<dyn MessageBroker> = Arc::new(MessageBrokerFactory::in_memory());
        let (received, ready) = spawn_subscriber(Arc::clone(&broker));
        ready
            .await
            .expect("subscribe must succeed against in-memory broker");

        broker
            .publish(TOPIC, Message::new(b"hello-subscriber".to_vec()))
            .await
            .expect("publish must succeed");

        wait_for_count(&received, 1).await;
        assert_eq!(received.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_spawn_subscriber_counts_multiple_messages_in_order_of_arrival() {
        let broker: Arc<dyn MessageBroker> = Arc::new(MessageBrokerFactory::in_memory());
        let (received, ready) = spawn_subscriber(Arc::clone(&broker));
        ready
            .await
            .expect("subscribe must succeed against in-memory broker");

        for i in 0..5u32 {
            broker
                .publish(TOPIC, Message::new(i.to_string().into_bytes()))
                .await
                .expect("publish must succeed");
        }

        wait_for_count(&received, 5).await;
        assert_eq!(received.load(Ordering::SeqCst), 5);
    }

    /// A message published *before* the subscriber's `subscribe()` call
    /// completes must never count — this is the documented semantics of
    /// `MessageBroker::subscribe`, not an implementation accident, so it is
    /// asserted directly rather than assumed.
    #[tokio::test]
    async fn test_message_published_before_subscribe_is_not_counted() {
        let broker: Arc<dyn MessageBroker> = Arc::new(MessageBrokerFactory::in_memory());

        // No subscriber exists yet — publish is fire-and-forget and must
        // silently succeed with nothing to deliver to.
        broker
            .publish(TOPIC, Message::new(b"too-early".to_vec()))
            .await
            .expect("publish with no subscribers must still succeed");

        let (received, ready) = spawn_subscriber(Arc::clone(&broker));
        ready
            .await
            .expect("subscribe must succeed against in-memory broker");

        broker
            .publish(TOPIC, Message::new(b"on-time".to_vec()))
            .await
            .expect("publish must succeed");

        wait_for_count(&received, 1).await;
        assert_eq!(
            received.load(Ordering::SeqCst),
            1,
            "only the message published after subscribe() completed should be counted"
        );
    }
}
