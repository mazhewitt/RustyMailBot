#!/bin/bash
set -e

# Check for required tools
if ! command -v helm &>/dev/null || ! command -v kubectl &>/dev/null; then
    echo "Error: helm and kubectl are required"
    exit 1
fi

# Define paths
LOCAL_ENV_FILE="$(pwd)/.env"

echo "Starting development environment..."

# Clean up existing resources
echo "Cleaning up previous deployment..."
helm uninstall dev-stack 2>/dev/null || true
kubectl delete job meilisearch-extract-keys 2>/dev/null || true
kubectl delete secret meilisearch-api-keys meilisearch-secret 2>/dev/null || true
sleep 5

# Deploy the dev-stack chart
echo "Deploying development stack..."
helm install dev-stack ./dev-stack

# Wait for the MeiliSearch pod to be ready
echo "Waiting for MeiliSearch pod to be ready..."
kubectl wait --for=condition=ready pod -l app=meilisearch --timeout=120s || true

echo "Waiting for Ollama pod to be ready..."
kubectl wait --for=condition=ready pod -l app=ollama --timeout=180s || true

# Wait for the API key extraction job to complete
echo "Waiting for the MeiliSearch key extraction job to complete..."
echo "(This may take up to 3 minutes...)"
kubectl wait --for=condition=complete job/meilisearch-extract-keys --timeout=180s || true

# Print job logs for debugging
echo "Job logs:"
kubectl logs job/meilisearch-extract-keys

# Get service ports (for Docker Desktop Kubernetes which uses NodePorts)
MEILI_PORT=$(kubectl get svc meilisearch -o jsonpath='{.spec.ports[0].nodePort}')
OLLAMA_PORT=$(kubectl get svc ollama -o jsonpath='{.spec.ports[0].nodePort}')

# Get the MeiliSearch API keys from the Kubernetes secret
echo "Extracting MeiliSearch API keys from Kubernetes secret..."
MEILI_SEARCH_KEY=$(kubectl get secret meilisearch-api-keys -o jsonpath='{.data.MEILI_SEARCH_KEY}' 2>/dev/null | base64 --decode) || MEILI_SEARCH_KEY=""
MEILI_ADMIN_KEY=$(kubectl get secret meilisearch-api-keys -o jsonpath='{.data.MEILI_ADMIN_KEY}' 2>/dev/null | base64 --decode) || MEILI_ADMIN_KEY=""

# Check if keys were found
if [ -z "$MEILI_SEARCH_KEY" ] || [ -z "$MEILI_ADMIN_KEY" ]; then
    echo "Warning: Failed to get MeiliSearch API keys from Kubernetes secret."
    echo "Attempting to extract keys directly from MeiliSearch..."

    # Get the master key
    MASTER_KEY=$(kubectl get secret meilisearch-secret -o jsonpath='{.data.MEILI_MASTER_KEY}' | base64 --decode)

    # Directly query MeiliSearch API
    API_KEYS=$(curl -s -H "Authorization: Bearer $MASTER_KEY" http://localhost:$MEILI_PORT/keys)
    MEILI_SEARCH_KEY=$(echo "$API_KEYS" | jq -r '.results[] | select(.name=="Default Search API Key") | .key')
    MEILI_ADMIN_KEY=$(echo "$API_KEYS" | jq -r '.results[] | select(.name=="Default Admin API Key") | .key')
fi

# Create a .env file with all the configuration for the local Rust application
echo "Creating .env file for your Rust application..."
cat > "${LOCAL_ENV_FILE}" <<EOF
# MeiliSearch Configuration
MEILI_URL=http://localhost:${MEILI_PORT}
MEILI_SEARCH_KEY=${MEILI_SEARCH_KEY}
MEILI_ADMIN_KEY=${MEILI_ADMIN_KEY}

# Ollama Configuration
OLLAMA_URL=http://localhost:${OLLAMA_PORT}
EOF

echo "Development environment is ready!"
echo "Your Rust application can connect to:"
echo "- MeiliSearch at http://localhost:${MEILI_PORT}"
echo "- Ollama at http://localhost:${OLLAMA_PORT}"
echo "API keys and connection information are stored in ${LOCAL_ENV_FILE}"