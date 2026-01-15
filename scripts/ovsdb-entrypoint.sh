#!/bin/sh
set -e

OVS_RUNDIR=/var/run/openvswitch
OVS_DBDIR=/etc/openvswitch
OVS_SCHEMA=/usr/share/openvswitch/vswitch.ovsschema
OVS_LOGDIR=/var/log/openvswitch
OPENFLOW_PORT=6653

# Create directories
mkdir -p "$OVS_RUNDIR" "$OVS_DBDIR" "$OVS_LOGDIR"

# Create database if it doesn't exist
if [ ! -f "$OVS_DBDIR/conf.db" ]; then
    echo "Creating OVSDB database..."
    ovsdb-tool create "$OVS_DBDIR/conf.db" "$OVS_SCHEMA"
fi

# Start ovsdb-server
echo "Starting ovsdb-server..."
ovsdb-server "$OVS_DBDIR/conf.db" \
    --remote=punix:"$OVS_RUNDIR/db.sock" \
    --remote=ptcp:6640:0.0.0.0 \
    --pidfile="$OVS_RUNDIR/ovsdb-server.pid" \
    --log-file="$OVS_LOGDIR/ovsdb-server.log" \
    --detach

# Wait for socket
echo "Waiting for OVSDB socket..."
i=0
while [ $i -lt 30 ]; do
    if [ -S "$OVS_RUNDIR/db.sock" ]; then
        break
    fi
    sleep 0.1
    i=$((i + 1))
done

# Initialize the database
echo "Initializing OVS database..."
ovs-vsctl --no-wait init

# Check if ovsdb-only mode requested
if [ "$1" = "ovsdb-only" ]; then
    echo "OVSDB-only mode (no ovs-vswitchd)"
    echo "OVS is ready!"
    echo "  OVSDB: tcp:0.0.0.0:6640"
    exec tail -f "$OVS_LOGDIR/ovsdb-server.log" 2>/dev/null || sleep infinity
fi

# Start ovs-vswitchd with userspace datapath (no kernel module needed)
echo "Starting ovs-vswitchd (userspace datapath)..."
ovs-vswitchd \
    --pidfile="$OVS_RUNDIR/ovs-vswitchd.pid" \
    --log-file="$OVS_LOGDIR/ovs-vswitchd.log" \
    --detach

# Wait for vswitchd to be ready
sleep 1

# Create a test bridge for OpenFlow testing
echo "Creating test bridge for OpenFlow..."
ovs-vsctl --may-exist add-br br-test -- \
    set bridge br-test datapath_type=netdev \
    protocols=OpenFlow10,OpenFlow13 \
    fail_mode=secure

# Set OpenFlow controller target (passive mode - we connect to it)
# The bridge listens on ptcp:6653 for controller connections
ovs-vsctl set-controller br-test ptcp:$OPENFLOW_PORT:0.0.0.0

# Add a couple of test ports
ovs-vsctl --may-exist add-port br-test test-port1 -- \
    set interface test-port1 type=internal
ovs-vsctl --may-exist add-port br-test test-port2 -- \
    set interface test-port2 type=internal

echo "OVS is ready!"
echo "  OVSDB: tcp:0.0.0.0:6640"
echo "  OVSDB: unix:$OVS_RUNDIR/db.sock"
echo "  OpenFlow: tcp:0.0.0.0:$OPENFLOW_PORT (br-test)"
echo "  vswitchd: running (userspace datapath)"
echo "  Bridge: br-test with ports test-port1, test-port2"

# Keep container running and show logs
exec tail -f "$OVS_LOGDIR/ovs-vswitchd.log" 2>/dev/null || sleep infinity
