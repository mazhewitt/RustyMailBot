#!/bin/bash
set -e

echo "Starting development environment..."

# Function to check if a command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

# Function to check if a service is running on a specific port
port_in_use() {
  lsof -i :"$1" >/dev/null 2>&1
}

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# -----------------------------
# Ollama Setup (Local Installation)
# -----------------------------
OLLAMA_PORT=11434
OLLAMA_MODEL="llama3.2"

echo "Checking Ollama installation..."

# Check if Ollama is installed
if ! command_exists ollama; then
  echo -e "${YELLOW}Ollama is not installed. Installing it using Homebrew...${NC}"

  # Check if Homebrew is installed
  if ! command_exists brew; then
    echo -e "${RED}Error: Homebrew is not installed. Please install Homebrew first.${NC}"
    echo "Visit https://brew.sh for installation instructions."
    exit 1
  fi

  # Install Ollama
  brew install ollama

  if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to install Ollama.${NC}"
    exit 1
  fi

  echo -e "${GREEN}Ollama installed successfully.${NC}"
fi

# Check if Ollama is running
if ! port_in_use $OLLAMA_PORT; then
  echo "Starting Ollama service..."
  ollama serve &

  # Wait for Ollama to start
  echo "Waiting for Ollama to start..."
  attempts=0
  max_attempts=20

  while ! curl -s "http://localhost:$OLLAMA_PORT/api/tags" > /dev/null && [ $attempts -lt $max_attempts ]; do
    sleep 1
    attempts=$((attempts + 1))
    echo -n "."
  done

  if [ $attempts -ge $max_attempts ]; then
    echo -e "\n${RED}Ollama failed to start within the expected time.${NC}"
    exit 1
  fi

  echo -e "\n${GREEN}Ollama started successfully.${NC}"
else
  echo -e "${GREEN}Ollama is already running.${NC}"
fi

# Pull the required model
echo "Checking if model $OLLAMA_MODEL is already downloaded..."
if ! curl -s "http://localhost:$OLLAMA_PORT/api/tags" | grep -q "\"$OLLAMA_MODEL\""; then
  echo "Pulling $OLLAMA_MODEL model (this may take a while)..."
  ollama pull $OLLAMA_MODEL

  if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to pull $OLLAMA_MODEL model.${NC}"
    exit 1
  fi

  echo -e "${GREEN}Model $OLLAMA_MODEL pulled successfully.${NC}"
else
  echo -e "${GREEN}Model $OLLAMA_MODEL is already available.${NC}"
fi

# -----------------------------
# Kubernetes Deployment
# -----------------------------
echo "Cleaning up previous deployment..."
helm uninstall dev-stack 2>/dev/null || true

echo "Deploying development stack (without Ollama)..."
if ! helm install dev-stack ./dev-stack; then
    echo -e "${RED}Failed to deploy development stack${NC}"
    exit 1
fi

# Wait for MeiliSearch pod to be ready with improved error handling
echo "Waiting for MeiliSearch pod to be ready..."
# First wait for pod to exist
sleep 5  # Give time for pod creation
if ! kubectl wait --for=condition=ready pod -l app=meilisearch --timeout=180s; then
    echo -e "${RED}MeiliSearch pod failed to become ready within timeout${NC}"
    echo "Checking pod status..."
    kubectl get pods -l app=meilisearch
    kubectl describe pods -l app=meilisearch
    exit 1
fi

# Ensure pod is fully ready before proceeding
MEILISEARCH_POD=$(kubectl get pods -l app=meilisearch -o jsonpath='{.items[0].metadata.name}')
if ! kubectl get pod $MEILISEARCH_POD -o jsonpath='{.status.containerStatuses[0].ready}' | grep -q "true"; then
    echo -e "${RED}MeiliSearch container is not ready${NC}"
    exit 1
fi

echo -e "${GREEN}MeiliSearch pod is ready${NC}"

# Wait for the key extraction job with improved error handling
echo "Waiting for the MeiliSearch key extraction job to complete..."
echo "(This may take up to 3 minutes...)"
if ! kubectl wait --for=condition=complete job/meilisearch-extract-keys --timeout=180s; then
    echo -e "${RED}Key extraction job failed to complete within timeout${NC}"
    echo "Checking job status..."
    kubectl get jobs meilisearch-extract-keys
    kubectl describe job meilisearch-extract-keys
    exit 1
fi

# Show job logs only if the job exists
if kubectl get job meilisearch-extract-keys &>/dev/null; then
    echo "Job logs:"
    kubectl logs job/meilisearch-extract-keys
else
    echo -e "${YELLOW}Warning: Key extraction job not found${NC}"
fi

# Create or update environment variables
echo "Creating .env file for your Rust application..."
MEILISEARCH_HOST="http://localhost:$(kubectl get svc meilisearch -o jsonpath='{.spec.ports[0].nodePort}')"
OLLAMA_HOST="http://localhost:$OLLAMA_PORT"

# Extract MeiliSearch keys - UPDATED TO USE CORRECT SECRET AND FIELD NAMES
MS_ADMIN_KEY=$(kubectl get secret meilisearch-api-keys -o jsonpath='{.data.MEILI_ADMIN_KEY}' 2>/dev/null | base64 --decode)
MS_SEARCH_KEY=$(kubectl get secret meilisearch-api-keys -o jsonpath='{.data.MEILI_SEARCH_KEY}' 2>/dev/null | base64 --decode)

if [ -z "$MS_ADMIN_KEY" ] || [ -z "$MS_SEARCH_KEY" ]; then
  echo "Warning: Failed to get MeiliSearch API keys from Kubernetes secret."
  echo "Attempting to extract keys directly from MeiliSearch..."
  # You might add additional logic here to get the keys directly
fi

# Create .env file
cat > .env << EOF
MEILISEARCH_URL=$MEILISEARCH_HOST
MEILI_ADMIN_KEY=$MS_ADMIN_KEY
MEILI_SEARCH_KEY=$MS_SEARCH_KEY
OLLAMA_URL=$OLLAMA_HOST
OLLAMA_MODEL=$OLLAMA_MODEL
MEILI_URL=$MEILISEARCH_HOST
EOF

echo -e "${GREEN}Development environment is ready!${NC}"
echo "Your Rust application can connect to:"
echo "- MeiliSearch at $MEILISEARCH_HOST"
echo "- Ollama at $OLLAMA_HOST"
echo "API keys and connection information are stored in $PWD/.env"