#!/usr/bin/env bash
set -euo pipefail
WORKSPACES=(
  autoscale cli configbuilder domain
  egress/database egress/grpc egress/http
  egress/message-broker egress/subprocess
  ingress ingress/grpc ingress/http
  message-broker "observ-config/swe-edge-observ-config"
  proxy runtime scheduler
)
for ws in "${WORKSPACES[@]}"; do
  echo "==> lint: $ws"
  (cd "$ws" && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings)
done
