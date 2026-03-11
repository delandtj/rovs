# OVS Unixctl Commands Reference

Reference for commands available via the `ovs-vswitchd` unixctl socket (the protocol used by `ovs-appctl`). These are the commands rovs can issue by connecting directly to the vswitchd control socket.

Socket path: `/var/run/openvswitch/ovs-vswitchd.<pid>.ctl`

Source: [ovs-vswitchd(8)](http://www.openvswitch.org/support/dist-docs/ovs-vswitchd.8.html)

---

## DPIF Commands (Datapath Interface)

Bridge-scoped datapath debugging commands.

| Command | Args | Description |
|---------|------|-------------|
| `dpif/dump-dps` | | List all configured datapaths |
| `dpif/show` | | Show datapaths with port info, OpenFlow numbers, and datapath port numbers |
| `dpif/dump-flows` | `[-m] <bridge>` | Dump all datapath flow entries for a bridge; `-m` includes wildcarded fields |
| `dpif/del-flows` | `<bridge>` | Delete all flow entries from a bridge's datapath flow table |

## DPCTL Commands (Datapath Control)

### Datapath Management

| Command | Args | Description |
|---------|------|-------------|
| `dpctl/add-dp` | `dp [netdev[,option]...]` | Create datapath with local port; optionally add netdevs |
| `dpctl/del-dp` | `dp` | Delete datapath and all associated netdevs |
| `dpctl/add-if` | `dp netdev[,option]...` | Add netdev to datapath (options: `type=`, `port_no=`, `key=value`) |
| `dpctl/set-if` | `dp port[,option]...` | Reconfigure datapath port options (`key=value` adds, `key=` deletes) |
| `dpctl/del-if` | `dp netdev...` | Remove netdev(s) from datapath |
| `dpctl/dump-dps` | | List all configured datapaths |
| `dpctl/show` | `[-s\|--statistics] [dp...]` | Show datapaths with ports; `-s` adds packet/byte counters |

### Flow Table

| Command | Args | Description |
|---------|------|-------------|
| `dpctl/dump-flows` | `[-m] [--names] [dp] [filter=] [type=] [pmd=]` | Dump all datapath flows; `type=` filters: ovs, tc, dpdk, offloaded, non-offloaded, partially-offloaded, all |
| `dpctl/add-flow` | `[dp] flow actions` | Add flow (fails if already exists) |
| `dpctl/mod-flow` | `[--clear] [--may-create] [-s] [dp] flow actions` | Modify flow; `--may-create` adds if missing; `-s` prints stats; `--clear` zeros stats |
| `dpctl/add-flows` | `[dp] file` | Bulk add flows from file (or stdin with `-`); lines may start with `add`, `modify`, `delete` |
| `dpctl/mod-flows` | `[dp] file` | Bulk modify flows from file |
| `dpctl/del-flow` | `[-s] [dp] flow` | Delete specific flow; `-s` prints stats before deletion |
| `dpctl/del-flows` | `[dp] [file]` | Delete all flows, or delete flows listed in file |
| `dpctl/get-flow` | `[dp] ufid:ufid [-m] [--names]` | Fetch flow by UFID (32-char hex) |

### Flow Cache

| Command | Args | Description |
|---------|------|-------------|
| `dpctl/cache-get-size` | `[dp]` | Print current cache sizes |
| `dpctl/cache-set-size` | `dp cache size` | Set cache size for a specific datapath |

### Connection Tracking

| Command | Args | Description |
|---------|------|-------------|
| `dpctl/dump-conntrack` | `[-m] [-s] [dp] [zone=]` | Dump all conntrack entries; `-m` for more detail; `-s` for stats; `zone=` to filter |
| `dpctl/dump-conntrack-exp` | `[dp] [zone=]` | Dump conntrack expectation entries (userspace only) |
| `dpctl/flush-conntrack` | `[dp] [zone=] [ct-origin-tuple [ct-reply-tuple]]` | Flush conntrack entries by zone and/or tuple match |
| `dpctl/ct-stats-show` | `[dp] [zone=] [-m]` | Connection counts grouped by protocol |
| `dpctl/ct-bkts` | `[dp] [gt=threshold]` | Per-bucket connection counts |
| `dpctl/ct-set-maxconns` | `[dp] maxconns` | Set max conntrack entries (userspace only) |
| `dpctl/ct-get-maxconns` | `[dp]` | Get max conntrack entries (userspace only) |
| `dpctl/ct-get-nconns` | `[dp]` | Get current conntrack entry count (userspace only) |
| `dpctl/ct-set-limits` | `[dp] [default=] [zone=,limit=]...` | Set per-zone connection limits |
| `dpctl/ct-del-limits` | `[dp] zone=zone[,zone]...` | Delete zone-specific limits |
| `dpctl/ct-get-limits` | `[dp] [zone=]` | Get zone limits and current usage |
| `dpctl/ct-enable-tcp-seq-chk` | `[dp]` | Enable TCP sequence verification (userspace only) |
| `dpctl/ct-disable-tcp-seq-chk` | `[dp]` | Disable TCP sequence verification (userspace only) |
| `dpctl/ct-get-tcp-seq-chk` | `[dp]` | Get TCP sequence check status (userspace only) |
| `dpctl/ct-set-sweep-interval` | `[dp] ms` | Set conntrack sweep interval in milliseconds (userspace only) |
| `dpctl/ct-get-sweep-interval` | `[dp]` | Get current sweep interval (userspace only) |

### IP Fragmentation (Userspace Only)

| Command | Args | Description |
|---------|------|-------------|
| `dpctl/ipf-set-enabled` | `[dp] v4\|v6` | Enable IP fragmentation handling |
| `dpctl/ipf-set-disabled` | `[dp] v4\|v6` | Disable IP fragmentation handling |
| `dpctl/ipf-set-min-frag` | `[dp] v4\|v6 minfrag` | Set minimum fragment size for non-final fragments |
| `dpctl/ipf-set-max-nfrags` | `[dp] maxfrags` | Set max tracked fragments |
| `dpctl/ipf-get-status` | `[dp] [-m]` | Get fragmentation config and counters |

## Other Notable Unixctl Commands

Commands outside of dpif/dpctl that may be relevant:

| Command | Args | Description |
|---------|------|-------------|
| `ofproto/trace` | `<bridge> <flow>` | Trace a packet through the OpenFlow pipeline |
| `fdb/show` | `<bridge>` | Show MAC learning table |
| `fdb/flush` | `[bridge]` | Flush MAC learning table |
| `fdb/stats-show` | `[bridge]` | Show MAC learning table stats |
| `coverage/show` | | Show coverage counters (internal stats) |
| `memory/show` | | Show memory usage |
| `upcall/show` | | Show upcall statistics |
| `list-commands` | | List all available commands |
