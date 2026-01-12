#!/bin/sh
set -e

OVS_RUNDIR=/var/run/openvswitch
OVS_DBDIR=/etc/openvswitch
OVS_SCHEMA=/usr/share/openvswitch/vswitch.ovsschema
OVS_LOGDIR=/var/log/openvswitch

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

echo "OVS is ready!"
echo "  OVSDB: tcp:0.0.0.0:6640"
echo "  OVSDB: unix:$OVS_RUNDIR/db.sock"
echo "  vswitchd: running (userspace datapath)"

# Keep container running and show logs
exec tail -f "$OVS_LOGDIR/ovs-vswitchd.log" 2>/dev/null || sleep infinity
