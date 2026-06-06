//! Integration tests — MessageBroker SAF surface exported from swe_edge_runtime.

use swe_edge_runtime_message_broker::MessageBroker;

#[cfg(feature = "message-broker")]
use swe_edge_bootstrap::MessageBrokerFactory;

#[test]
fn test_message_broker_trait_is_object_safe_in_runtime() {
    fn _assert(_: &dyn MessageBroker) {}
}

#[cfg(feature = "message-broker")]
#[test]
fn test_in_memory_broker_factory_is_exported_from_runtime() {
    let broker = MessageBrokerFactory::in_memory();
    let _: &dyn MessageBroker = &broker;
}

#[cfg(feature = "message-broker")]
#[tokio::test]
async fn test_in_memory_broker_health_check_returns_ok() {
    let broker = MessageBrokerFactory::in_memory();
    assert!(broker.health_check().await.is_ok());
}
