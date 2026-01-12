#!/bin/sh
# Run rovs tests against OVS in a container
#
# Usage:
#   ./scripts/test-with-ovs.sh              # Run all tests (unit + integration)
#   ./scripts/test-with-ovs.sh unit         # Run only unit tests (no container)
#   ./scripts/test-with-ovs.sh integration  # Run only integration tests
#   ./scripts/test-with-ovs.sh examples     # Run examples
#   ./scripts/test-with-ovs.sh full         # Run with ovs-vswitchd (privileged)

set -e

CONTAINER_NAME="rovs-ovsdb-test"
IMAGE_NAME="rovs-ovsdb"
OVSDB_PORT=6640

# Colors (if terminal supports it)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    NC=''
fi

log_info() { printf "${GREEN}[INFO]${NC} %s\n" "$1"; }
log_warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
log_error() { printf "${RED}[ERROR]${NC} %s\n" "$1"; }

cleanup() {
    if podman ps -q -f name="$CONTAINER_NAME" 2>/dev/null | grep -q .; then
        log_info "Stopping container..."
        podman stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi
}

build_image() {
    if ! podman image exists "$IMAGE_NAME" 2>/dev/null; then
        log_info "Building OVS container image..."
        podman build -t "$IMAGE_NAME" .
    else
        log_info "Using existing image: $IMAGE_NAME"
    fi
}

start_container() {
    local mode="${1:-ovsdb-only}"
    local extra_args=""

    cleanup

    if [ "$mode" = "full" ]; then
        log_info "Starting OVS container (full mode with ovs-vswitchd)..."
        extra_args="--privileged"
    else
        log_info "Starting OVS container (ovsdb-only mode)..."
    fi

    podman run --rm -d \
        $extra_args \
        -p "$OVSDB_PORT:6640" \
        --name "$CONTAINER_NAME" \
        "$IMAGE_NAME" \
        $([ "$mode" = "ovsdb-only" ] && echo "ovsdb-only")

    # Wait for OVSDB to be ready
    log_info "Waiting for OVSDB to be ready..."
    for i in $(seq 1 30); do
        if podman exec "$CONTAINER_NAME" ovs-vsctl show >/dev/null 2>&1; then
            log_info "OVSDB is ready!"
            return 0
        fi
        sleep 0.5
    done

    log_error "OVSDB failed to start"
    podman logs "$CONTAINER_NAME"
    exit 1
}

run_unit_tests() {
    log_info "Running unit tests..."
    cargo test --lib --all
}

run_integration_tests() {
    log_info "Running integration tests..."
    # --test-threads=1 required: tests share one OVSDB connection and would steal each other's update notifications
    OVSDB_ADDR="tcp:127.0.0.1:$OVSDB_PORT" cargo test -- --ignored --test-threads=1
}

run_examples() {
    log_info "Running examples..."
    OVSDB_ADDR="tcp:127.0.0.1:$OVSDB_PORT" cargo run --example ovsdb_transaction
    OVSDB_ADDR="tcp:127.0.0.1:$OVSDB_PORT" cargo run --example list_bridges
}

run_all() {
    run_unit_tests
    run_integration_tests
}

# Trap to cleanup on exit
trap cleanup EXIT

case "${1:-all}" in
    unit)
        run_unit_tests
        ;;
    integration)
        build_image
        start_container "ovsdb-only"
        run_integration_tests
        ;;
    examples)
        build_image
        start_container "ovsdb-only"
        run_examples
        ;;
    full)
        build_image
        start_container "full"
        run_integration_tests
        run_examples
        ;;
    all)
        build_image
        start_container "ovsdb-only"
        run_all
        ;;
    build)
        build_image
        ;;
    start)
        build_image
        start_container "${2:-ovsdb-only}"
        log_info "Container running. OVSDB_ADDR=tcp:127.0.0.1:$OVSDB_PORT"
        log_info "Press Ctrl+C to stop"
        trap - EXIT
        podman logs -f "$CONTAINER_NAME"
        ;;
    stop)
        cleanup
        ;;
    *)
        echo "Usage: $0 {all|unit|integration|examples|full|build|start|stop}"
        echo ""
        echo "  all          Run unit + integration tests (default)"
        echo "  unit         Run unit tests only (no container)"
        echo "  integration  Run integration tests against container"
        echo "  examples     Run examples against container"
        echo "  full         Run with ovs-vswitchd (privileged container)"
        echo "  build        Build container image only"
        echo "  start        Start container and keep running"
        echo "  stop         Stop running container"
        exit 1
        ;;
esac

log_info "Done!"
