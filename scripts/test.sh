#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-native}"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cleanup() {
    echo "Cleaning up..."
    if [ "$MODE" = "docker" ]; then
        docker compose -f "$PROJECT_DIR/docker-compose.full.yml" down -v --remove-orphans 2>/dev/null || true
    else
        docker compose -f "$PROJECT_DIR/docker-compose.yml" down -v --remove-orphans 2>/dev/null || true
    fi
}

trap cleanup EXIT

wait_for_services() {
    local compose_file="$1"
    local max_attempts=60
    local attempt=0

    echo "Waiting for services to be healthy..."
    while [ $attempt -lt $max_attempts ]; do
        if docker compose -f "$compose_file" ps --format json 2>/dev/null | \
            grep -q '"Health":"unhealthy"'; then
            attempt=$((attempt + 1))
            sleep 2
            continue
        fi

        # Check that all services with healthchecks are healthy
        local unhealthy
        unhealthy=$(docker compose -f "$compose_file" ps --format json 2>/dev/null | \
            grep -c '"Health":"starting"' || true)
        if [ "$unhealthy" -eq 0 ]; then
            echo "All services are healthy."
            return 0
        fi

        attempt=$((attempt + 1))
        sleep 2
    done

    echo "Error: Services did not become healthy within $((max_attempts * 2)) seconds."
    docker compose -f "$compose_file" ps
    docker compose -f "$compose_file" logs
    return 1
}

run_tests() {
    echo "Running cargo tests..."
    cargo test --workspace

    echo "Running e2e tests..."
    cargo test --package purestat-tests
}

case "$MODE" in
    docker)
        echo "Starting full Docker stack..."
        docker compose -f "$PROJECT_DIR/docker-compose.full.yml" up -d --build
        wait_for_services "$PROJECT_DIR/docker-compose.full.yml"
        run_tests
        ;;
    native)
        echo "Starting infrastructure services..."
        docker compose -f "$PROJECT_DIR/docker-compose.yml" up -d
        wait_for_services "$PROJECT_DIR/docker-compose.yml"
        run_tests
        ;;
    *)
        echo "Usage: $0 [native|docker]"
        echo "  native  - Start infra via docker-compose.yml, run tests natively (default)"
        echo "  docker  - Start full stack via docker-compose.full.yml, run tests"
        exit 1
        ;;
esac

echo "All tests passed."
