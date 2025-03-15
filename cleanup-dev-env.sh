#!/bin/bash
kubectl delete job meilisearch-extract-keys
helm upgrade --install dev-stack ./dev-stack \
  --set meilisearch.devMode=true \
  --set meilisearch.devSecretPath="$(pwd)/meilisearch_keys.env"