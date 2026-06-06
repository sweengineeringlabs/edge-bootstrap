#!/usr/bin/env bash
set -euo pipefail
for ws in autoscale config/swe-edge-config domain egress/grpc egress/http           ingress/grpc ingress/http ingress/tls ingress/verifier           observ-config/swe-edge-observ-config proxy runtime; do
  echo "==> test: $ws"
  (cd "$ws" && cargo test --workspace --locked)
done
