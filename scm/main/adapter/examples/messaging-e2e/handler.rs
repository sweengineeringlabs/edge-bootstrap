//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the order-intake handler look like, and where does it
//! publish to."
//!
//! `swe_edge_bootstrap::RuntimeBuilder::with_message_broker` only wires a
//! broker in for startup health-check reporting (probed once, folded into
//! `RuntimeHealth`) — it never reaches `HandlerContext`/`ExecutionRequest`
//! the way `observer` does, so there is no framework seam a handler can pull
//! a broker out of. Instead this handler takes its own `Arc<dyn
//! MessageBroker>` as a plain constructor field, the same dependency
//! injection pattern any handler would use for a database pool.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};

#[cfg(feature = "message-broker")]
use swe_edge_bootstrap::{Message, MessageBroker};

/// An incoming order, decoded from the JSON body of a `POST /orders` request.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct OrderPayload {
    pub(crate) id: String,
}
impl edge_application::Request for OrderPayload {}
impl edge_application::Response for OrderPayload {}

struct OrderHandler {
    #[cfg(feature = "message-broker")]
    broker: Arc<dyn MessageBroker>,
}

#[async_trait]
impl Handler for OrderHandler {
    type Request = OrderPayload;
    type Response = OrderPayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/orders".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/orders".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, OrderPayload>,
    ) -> Result<OrderPayload, HandlerError> {
        let order = req.req;

        #[cfg(feature = "message-broker")]
        {
            let payload = serde_json::to_vec(&order)
                .map_err(|e| HandlerError::ExecutionFailed(e.to_string()))?;
            if let Err(err) = self
                .broker
                .publish(crate::broker::TOPIC, Message::new(payload))
                .await
            {
                // A failed publish doesn't fail the request — the order was
                // still accepted — but it must not be swallowed silently.
                eprintln!(
                    "messaging: publish to '{}' failed: {err}",
                    crate::broker::TOPIC
                );
            }
        }
        #[cfg(not(feature = "message-broker"))]
        {
            println!(
                "messaging: message-broker feature disabled — order '{}' accepted but not published",
                order.id
            );
        }

        Ok(order)
    }
}

/// Build the order-intake `Handler`, served over both HTTP and gRPC.
#[cfg(feature = "message-broker")]
pub(crate) fn build_handler(
    broker: Arc<dyn MessageBroker>,
) -> Arc<dyn Handler<Request = OrderPayload, Response = OrderPayload>> {
    Arc::new(OrderHandler { broker })
}

/// Build the order-intake `Handler` with no broker at all — the
/// `message-broker` feature is off, so publishing is a genuine no-op rather
/// than a stub call into a type that doesn't exist in this build.
#[cfg(not(feature = "message-broker"))]
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = OrderPayload, Response = OrderPayload>> {
    Arc::new(OrderHandler {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::{HandlerContext, SecurityContext};
    use edge_application_command::NoopCommandBus;
    use edge_application_observer::StdObserveFactory;

    fn execution_context() -> (
        NoopCommandBus,
        SecurityContext,
        Arc<dyn edge_application_observer::ObserverContext>,
    ) {
        (
            NoopCommandBus,
            SecurityContext::unauthenticated(),
            StdObserveFactory::noop_arc_observe_context(),
        )
    }

    #[test]
    fn test_id_returns_orders_path() {
        let handler = OrderHandler {
            #[cfg(feature = "message-broker")]
            broker: Arc::new(swe_edge_bootstrap::MessageBrokerFactory::in_memory()),
        };
        assert_eq!(handler.id(IdRequest).unwrap().id, "/orders");
    }

    #[test]
    fn test_pattern_returns_orders_path() {
        let handler = OrderHandler {
            #[cfg(feature = "message-broker")]
            broker: Arc::new(swe_edge_bootstrap::MessageBrokerFactory::in_memory()),
        };
        assert_eq!(handler.pattern(PatternRequest).unwrap().pattern, "/orders");
    }

    #[tokio::test]
    async fn test_execute_returns_the_order_unchanged() {
        let handler = OrderHandler {
            #[cfg(feature = "message-broker")]
            broker: Arc::new(swe_edge_bootstrap::MessageBrokerFactory::in_memory()),
        };
        let (commands, security, observer) = execution_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let order = OrderPayload {
            id: "order-42".to_string(),
        };
        let resp = handler
            .execute(ExecutionRequest {
                req: order.clone(),
                ctx: &ctx,
            })
            .await
            .expect("execute must succeed");
        assert_eq!(resp.id, order.id);
    }

    #[cfg(feature = "message-broker")]
    #[tokio::test]
    async fn test_execute_publishes_the_order_to_the_broker() {
        use futures::StreamExt;

        let broker: Arc<dyn MessageBroker> =
            Arc::new(swe_edge_bootstrap::MessageBrokerFactory::in_memory());
        let mut stream = broker
            .subscribe(crate::broker::TOPIC)
            .await
            .expect("subscribe must succeed");

        let handler = build_handler(Arc::clone(&broker));
        let (commands, security, observer) = execution_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let order = OrderPayload {
            id: "order-77".to_string(),
        };
        handler
            .execute(ExecutionRequest {
                req: order.clone(),
                ctx: &ctx,
            })
            .await
            .expect("execute must succeed");

        let received = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("timed out waiting for the handler's publish")
            .expect("stream ended without a message")
            .expect("message must decode without a broker error");
        let decoded: OrderPayload =
            serde_json::from_slice(&received.payload).expect("payload must be valid JSON");
        assert_eq!(decoded.id, order.id);
    }
}
