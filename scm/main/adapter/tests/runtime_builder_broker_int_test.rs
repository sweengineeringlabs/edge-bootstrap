//! Tests for RuntimeBuilder with message broker.

#[cfg(feature = "message-broker")]
/// @covers: with_message_broker
#[test]
fn test_with_message_broker_sets_field() {
    use futures;
    use swe_edge_bootstrap::Runtime;
    use swe_edge_runtime_message_broker::{BrokerError, Message, MessageBroker, MessageStream};

    struct RuntimeBuilderStubBroker;
    impl MessageBroker for RuntimeBuilderStubBroker {
        fn publish<'a>(
            &'a self,
            _: &'a str,
            _: Message,
        ) -> futures::future::BoxFuture<'a, Result<(), BrokerError>> {
            Box::pin(futures::future::ready(Ok(())))
        }
        fn subscribe<'a>(
            &'a self,
            _: &'a str,
        ) -> futures::future::BoxFuture<'a, Result<MessageStream, BrokerError>> {
            Box::pin(futures::future::ready(Ok(
                Box::pin(futures::stream::empty()) as MessageStream,
            )))
        }
        fn health_check(&self) -> futures::future::BoxFuture<'_, Result<(), BrokerError>> {
            Box::pin(futures::future::ready(Ok(())))
        }
    }
    let _b = Runtime::builder().with_message_broker(RuntimeBuilderStubBroker);
    // Test passes if it doesn't panic when setting the message broker
}
