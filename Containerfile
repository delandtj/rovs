# OVS test environment with userspace datapath for rovs integration tests
#
# Build:
#   podman build -t rovs-ovsdb .
#
# Run (privileged for ovs-vswitchd userspace datapath):
#   podman run --rm -d --privileged -p 6640:6640 --name rovs-ovsdb rovs-ovsdb
#
# Run (minimal caps, OVSDB-only, no vswitchd):
#   podman run --rm -d -p 6640:6640 --name rovs-ovsdb rovs-ovsdb ovsdb-only
#
# Test:
#   OVSDB_ADDR=tcp:127.0.0.1:6640 cargo test -- --ignored

FROM docker.io/library/alpine:3.21

RUN apk add --no-cache \
    openvswitch \
    bash \
    iproute2 \
    && mkdir -p /var/run/openvswitch /etc/openvswitch /var/log/openvswitch

COPY scripts/ovsdb-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# OVSDB listens on TCP 6640, OpenFlow on 6653
EXPOSE 6640 6653

# Health check - verify ovsdb-server is responding
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=3 \
    CMD ovs-vsctl show >/dev/null 2>&1 || exit 1

ENTRYPOINT ["/entrypoint.sh"]
