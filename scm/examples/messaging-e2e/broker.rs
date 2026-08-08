//! Broker construction — the one thing this module owns is "what pub/sub
//! backend does this demo actually talk to, and how does it degrade when
//! that backend isn't compiled in."
//!
//! Three tiers, selected purely by Cargo feature (no runtime flag):
//!   - `message-broker-nats` (implies `message-broker`): connects to a real
//!     `nats-server` at [`NATS_URL`] via `MessageBrokerFactory::nats`, the
//!     "Kafka alternative" backend this workspace already ships. Falls back
//!     to the in-memory backend if the connection attempt itself fails, so a
//!     dead `nats-server` degrades the demo rather than crashing it.
//!   - `message-broker` alone: no network backend is compiled in at all
//!     (`swe-edge-runtime-message-broker-adapter/nats` never got turned on),
//!     so [`build_broker`] always constructs
//!     `MessageBrokerFactory::in_memory()` — a real broker, just
//!     process-local.
//!   - neither feature: this module doesn't exist; [`handler`](crate::handler)
//!     and [`subscriber`](crate::subscriber) short-circuit to a no-op instead.

use std::sync::Arc;

use swe_edge_bootstrap::{MessageBroker, MessageBrokerFactory};

/// Topic the handler publishes to and the background subscriber listens on.
pub(crate) const TOPIC: &str = "orders.created";

/// Local `nats-server` address. The default install listens here with no
/// special configuration required.
#[cfg(feature = "message-broker-nats")]
pub(crate) const NATS_URL: &str = "127.0.0.1:4222";

/// Build the broker the handler publishes through and the background
/// subscriber reads from — both sides of this demo's one real connection
/// (or, absent `nats`, one real in-memory channel registry).
pub(crate) async fn build_broker() -> Arc<dyn MessageBroker> {
    #[cfg(feature = "message-broker-nats")]
    {
        match MessageBrokerFactory::nats(NATS_URL).await {
            Ok(broker) => {
                println!("messaging: connected to nats-server at {NATS_URL}");
                return Arc::new(broker);
            }
            Err(err) => {
                eprintln!(
                    "messaging: nats connect to {NATS_URL} failed ({err}); \
                     falling back to the in-memory broker"
                );
            }
        }
    }
    Arc::new(MessageBrokerFactory::in_memory())
}

/// Build a second, independent broker instance purely to hand to
/// [`swe_edge_bootstrap::RuntimeBuilder::with_message_broker`], proving that
/// seam separately from the handler/subscriber's own connection — it is
/// probed once at startup and folded into `RuntimeHealth`, nothing more, so
/// the in-memory backend is the clearer choice here regardless of which
/// backend [`build_broker`] picked.
pub(crate) fn build_health_check_broker() -> impl MessageBroker {
    MessageBrokerFactory::in_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises `build_broker()` end to end: whatever backend it actually
    /// picked (real NATS if reachable, in-memory otherwise — the fallback
    /// makes this deterministic even when no `nats-server` is running), a
    /// subscriber established beforehand must receive a message published
    /// afterward. A broker that publishes into a void or never delivers
    /// would fail this.
    #[tokio::test]
    async fn test_build_broker_round_trips_a_published_message() {
        let broker = build_broker().await;
        let topic = "messaging-e2e-test.build-broker";

        let mut stream = broker
            .subscribe(topic)
            .await
            .expect("subscribe must succeed");

        broker
            .publish(
                topic,
                swe_edge_bootstrap::Message::new(b"round-trip".to_vec()),
            )
            .await
            .expect("publish must succeed");

        use futures::StreamExt;
        let received = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("timed out waiting for the published message")
            .expect("stream ended without yielding a message")
            .expect("message must decode without a broker error");
        assert_eq!(received.payload.as_ref(), b"round-trip");
    }

    #[tokio::test]
    async fn test_build_health_check_broker_reports_healthy() {
        let broker = build_health_check_broker();
        broker
            .health_check()
            .await
            .expect("in-memory broker's health check must always succeed");
    }

    #[test]
    fn test_topic_is_non_empty_and_stable() {
        assert_eq!(TOPIC, "orders.created");
    }
}
