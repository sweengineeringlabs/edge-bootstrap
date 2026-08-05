//! Order-intake `Handler` served over HTTP *and* gRPC, publishing every
//! accepted order to a real pub/sub broker — the "Kafka alternative" this
//! workspace already ships (`swe-edge-runtime-message-broker-adapter`) —
//! with a background subscriber proving the round trip actually happens.
//!
//! Run against a real local NATS server (the default install needs no
//! special configuration — `nats-server` alone is enough):
//!     nats-server &
//!     cargo run -p swe-edge-bootstrap --example messaging-e2e --features message-broker-nats
//!
//! Run against the in-process broker instead (no external process needed):
//!     cargo run -p swe-edge-bootstrap --example messaging-e2e --features message-broker
//!
//! Run with messaging compiled out entirely (the handler still serves, it
//! just never publishes):
//!     cargo run -p swe-edge-bootstrap --example messaging-e2e
//!
//! Unlike `bootstrap-e2e` (which wires metrics, egress, IDS/IPS, and
//! tracing through `RuntimeBuilder`), this example is scoped to one thing:
//! proving `swe-edge-runtime-message-broker-adapter`'s `MessageBroker` is a
//! real, live dependency, not a declared-and-unused one.
//!
//!   1. [`broker`] owns backend selection: a real NATS connection when
//!      `message-broker-nats` is compiled in (falling back to the
//!      in-memory backend if `nats-server` isn't reachable), or the
//!      in-memory backend outright when only `message-broker` is on.
//!   2. [`handler`]'s `OrderHandler` takes an `Arc<dyn MessageBroker>` as a
//!      plain constructor field and publishes to `broker::TOPIC` on every
//!      `execute()` — `RuntimeBuilder::with_message_broker` doesn't reach
//!      `HandlerContext` the way `observer` does (it only wires a broker in
//!      for startup health-check reporting), so there is no framework seam
//!      to pull one out of; this is the same injection pattern any handler
//!      would use for a database pool.
//!   3. [`subscriber`] subscribes to the *same* broker `Arc` the handler
//!      publishes through, on a background task, and counts every message
//!      it actually receives — proving genuine round-trip pub/sub, not
//!      just that `publish()` didn't return an error.
//!   4. A *second*, independent broker is registered via
//!      `.with_message_broker(...)` purely to demonstrate that seam too —
//!      probed once at startup and folded into `RuntimeHealth`.
//!   5. [`runtime_config`] owns bind addresses.
//!
//! Split by responsibility rather than one `main()` doing everything, same
//! as `bootstrap-e2e`: each module owns exactly one piece of the wiring,
//! `main` only assembles them and serves.

#[cfg(feature = "message-broker")]
mod broker;
mod handler;
mod runtime_config;
#[cfg(feature = "message-broker")]
mod subscriber;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    #[cfg(feature = "message-broker")]
    let primary_broker = broker::build_broker().await;

    #[cfg(feature = "message-broker")]
    let handler = handler::build_handler(std::sync::Arc::clone(&primary_broker));
    #[cfg(not(feature = "message-broker"))]
    let handler = handler::build_handler();

    #[cfg(feature = "message-broker")]
    let (received, subscriber_ready) =
        subscriber::spawn_subscriber(std::sync::Arc::clone(&primary_broker));

    // `MessageBroker::subscribe`'s contract only delivers messages published
    // *after* subscription is established — block here until the background
    // task's `subscribe()` call has actually completed, so no request
    // handled below can race ahead of it.
    #[cfg(feature = "message-broker")]
    {
        if subscriber_ready.await.is_err() {
            eprintln!(
                "messaging: background subscriber failed to start; \
                 published orders will not be observed"
            );
        } else {
            println!(
                "messaging: background subscriber is live on '{}'",
                broker::TOPIC
            );
        }
    }

    let config = runtime_config::build_runtime_config();

    println!(
        "http: http://{}/orders   (POST {{\"id\": \"...\"}})",
        runtime_config::HTTP_BIND
    );
    println!("grpc: {} (reflection off)", runtime_config::GRPC_BIND);

    let builder = Runtime::builder()
        .config(config)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated();

    // `.with_message_broker()` only exists behind the `message-broker`
    // feature — without it, the runtime health report has no
    // `"message-broker"` component at all.
    #[cfg(feature = "message-broker")]
    let builder = builder.with_message_broker(broker::build_health_check_broker());

    let result = builder.serve().await;

    #[cfg(feature = "message-broker")]
    println!(
        "messaging: subscriber received {} message(s) total before shutdown",
        received.load(std::sync::atomic::Ordering::SeqCst)
    );

    if let Err(e) = result {
        panic!("serve failed: {e}");
    }
}
