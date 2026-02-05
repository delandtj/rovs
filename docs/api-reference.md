# rovs API Reference

Complete API documentation for all rovs crates.

## Table of Contents

- [rovs-transport](#rovs-transport) - Network transport layer
- [rovs-jsonrpc](#rovs-jsonrpc) - JSON-RPC 1.0 protocol
- [rovs-ovsdb](#rovs-ovsdb) - OVSDB client and IDL
- [rovs-openflow](#rovs-openflow) - OpenFlow 1.3 + Nicira extensions
- [rovs-ext](#rovs-ext) - High-level abstractions
- [rovs-types](#rovs-types) - Shared types

---

## rovs-transport

Network transport layer for Unix sockets, TCP, and TLS connections.

### Address

```rust
pub enum Address {
    Unix(PathBuf),
    Tcp(SocketAddr),
    Ssl { addr: SocketAddr, server_name: String },
}

impl Address {
    pub fn parse(s: &str) -> Result<Self, Error>;
}

impl FromStr for Address { ... }
```

### Stream

```rust
pub enum Stream {
    Unix(UnixStream),
    Tcp(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Stream {
    pub async fn connect(addr: &Address) -> Result<Self, Error>;
    pub async fn connect_tls(addr: &Address, config: TlsConfig) -> Result<Self, Error>;
}
```

---

## rovs-jsonrpc

JSON-RPC 1.0 implementation for OVSDB protocol.

### Connection

```rust
pub struct Connection { ... }

impl Connection {
    pub fn new(stream: Stream) -> Self;
    pub fn next_id(&self) -> u64;
    pub async fn transact(&mut self, method: &str, params: Value) -> Result<Value>;
    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()>;
    pub async fn recv_message(&mut self) -> Result<Message>;
    pub fn pop_notification(&mut self) -> Option<Request>;
}
```

---

## rovs-ovsdb

OVSDB client with IDL and transaction support.

### Client

```rust
pub struct Client { ... }

impl Client {
    pub async fn connect(addr: &str) -> Result<Self>;
    pub async fn connect_with_config(addr: &str, config: ClientConfig) -> Result<Self>;
    pub fn idl(&self) -> &Idl;
    pub fn schema(&self) -> Option<&DbSchema>;
    pub async fn run(&mut self) -> Result<bool>;
    pub async fn wait(&mut self) -> Result<()>;
    pub async fn commit(&mut self, txn: &mut Transaction) -> Result<bool>;
}
```

### Transaction

```rust
pub struct Transaction { ... }

impl Transaction {
    pub fn new(db_name: impl Into<String>) -> Self;

    // Basic operations
    pub fn insert(&mut self, table: &str, row: Value) -> RowRef;
    pub fn update(&mut self, table: &str, uuid: Uuid, columns: Value);
    pub fn delete(&mut self, table: &str, uuid: Uuid);

    // High-level topology
    pub fn create_bridge(&mut self, name: &str) -> (RowRef, RowRef, RowRef);
    pub fn add_port(&mut self, bridge: &str, port: &str, iface_type: &str) -> (RowRef, RowRef);
    pub fn add_internal_port(&mut self, bridge: &str, port: &str) -> (RowRef, RowRef);
    pub fn add_vlan_port(&mut self, bridge: &str, port: &str, vlan: u16) -> (RowRef, RowRef);
    pub fn add_patch_ports(&mut self, br1: &str, br2: &str, ...) -> (RowRef, RowRef, RowRef, RowRef);
    pub fn set_controller(&mut self, bridge: &str, target: &str);

    // Results
    pub fn uuid_map(&self) -> &HashMap<String, Uuid>;
    pub fn get_uuid(&self, name: &str) -> Option<Uuid>;
}
```

### Idl

```rust
pub struct Idl { ... }

impl Idl {
    pub fn rows(&self, table: &str) -> impl Iterator<Item = &Row>;
    pub fn get_row(&self, table: &str, uuid: &Uuid) -> Option<&Row>;
    pub fn find_row<F>(&self, table: &str, predicate: F) -> Option<&Row>;
    pub fn change_seqno(&self) -> u64;
}
```

### Row

```rust
pub struct Row { ... }

impl Row {
    pub fn uuid(&self) -> Uuid;
    pub fn get(&self, column: &str) -> Option<&Atom>;
    pub fn get_string(&self, column: &str) -> Option<&str>;
    pub fn get_i64(&self, column: &str) -> Option<i64>;
    pub fn get_uuid(&self, column: &str) -> Option<Uuid>;
    pub fn get_set(&self, column: &str) -> Vec<Atom>;
    pub fn get_map(&self, column: &str) -> Vec<(Atom, Atom)>;
}
```

---

## rovs-openflow

OpenFlow 1.3 protocol with Nicira extensions.

### VConn (Virtual Connection)

```rust
pub struct VConn { ... }

impl VConn {
    pub async fn connect(addr: &Address) -> Result<Self>;
    pub fn version(&self) -> OfVersion;

    // Flow operations
    pub async fn send_flow(&mut self, flow: &Flow) -> Result<()>;
    pub async fn send_flow_sync(&mut self, flow: &Flow) -> Result<()>;
    pub async fn dump_flows(&mut self) -> Result<Vec<FlowStats>>;

    // Packet operations
    pub async fn recv_packet_in(&mut self) -> Result<PacketIn>;
    pub async fn send_packet_out(&mut self, packet: &PacketOut) -> Result<()>;
}
```

### Flow Builder

```rust
pub struct Flow { ... }

impl Flow {
    pub fn add() -> Self;
    pub fn modify() -> Self;
    pub fn modify_strict() -> Self;
    pub fn delete() -> Self;

    pub fn table(self, id: u8) -> Self;
    pub fn priority(self, priority: u16) -> Self;
    pub fn cookie(self, cookie: u64) -> Self;
    pub fn idle_timeout(self, secs: u16) -> Self;
    pub fn hard_timeout(self, secs: u16) -> Self;
    pub fn match_fields(self, m: Match) -> Self;
    pub fn actions(self, actions: ActionList) -> Self;
}
```

### Match Builder

```rust
pub struct Match { ... }

impl Match {
    pub fn new() -> Self;

    // Layer 2
    pub fn in_port(self, port: u32) -> Self;
    pub fn eth_type(self, ethertype: u16) -> Self;
    pub fn eth_src(self, mac: [u8; 6]) -> Self;
    pub fn eth_dst(self, mac: [u8; 6]) -> Self;
    pub fn vlan_vid(self, vid: u16) -> Self;

    // Layer 3 - IPv4
    pub fn ipv4_src(self, addr: Ipv4Addr) -> Self;
    pub fn ipv4_src_masked(self, addr: Ipv4Addr, mask: Ipv4Addr) -> Self;
    pub fn ipv4_dst(self, addr: Ipv4Addr) -> Self;
    pub fn ip_proto(self, proto: u8) -> Self;

    // Layer 3 - IPv6
    pub fn ipv6_src(self, addr: Ipv6Addr) -> Self;
    pub fn ipv6_dst(self, addr: Ipv6Addr) -> Self;

    // Layer 4
    pub fn tcp_src(self, port: u16) -> Self;
    pub fn tcp_dst(self, port: u16) -> Self;
    pub fn udp_src(self, port: u16) -> Self;
    pub fn udp_dst(self, port: u16) -> Self;

    // ARP
    pub fn arp_op(self, op: u16) -> Self;
    pub fn arp_spa(self, addr: Ipv4Addr) -> Self;
    pub fn arp_tpa(self, addr: Ipv4Addr) -> Self;

    // ICMPv6/NDP
    pub fn icmpv6_type(self, t: u8) -> Self;
    pub fn icmpv6_code(self, c: u8) -> Self;
    pub fn ipv6_nd_target(self, addr: Ipv6Addr) -> Self;

    // Connection tracking
    pub fn ct_state(self, state: u32) -> Self;
    pub fn ct_state_masked(self, state: u32, mask: u32) -> Self;
    pub fn ct_zone(self, zone: u16) -> Self;
    pub fn ct_mark(self, mark: u32) -> Self;
}
```

### ActionList Builder

```rust
pub struct ActionList { ... }

impl ActionList {
    pub fn new() -> Self;

    // Basic actions
    pub fn output(self, port: u32) -> Self;
    pub fn output_in_port(self) -> Self;
    pub fn controller(self, max_len: u16) -> Self;
    pub fn drop(self) -> Self;
    pub fn normal(self) -> Self;
    pub fn flood(self) -> Self;

    // VLAN actions
    pub fn push_vlan(self, ethertype: u16) -> Self;
    pub fn pop_vlan(self) -> Self;
    pub fn set_vlan_vid(self, vid: u16) -> Self;

    // Set field actions
    pub fn set_eth_src(self, mac: [u8; 6]) -> Self;
    pub fn set_eth_dst(self, mac: [u8; 6]) -> Self;
    pub fn set_ipv4_src(self, addr: Ipv4Addr) -> Self;
    pub fn set_ipv4_dst(self, addr: Ipv4Addr) -> Self;

    // Connection tracking
    pub fn ct(self, flags: u16, zone: u16, table: Option<u8>) -> Self;
    pub fn ct_nat(self, flags: u16, zone: u16, table: Option<u8>, nat: NatConfig) -> Self;
    pub fn ct_snat(self, zone: u16, table: Option<u8>, addr: Ipv4Addr) -> Self;
    pub fn ct_dnat(self, zone: u16, table: Option<u8>, addr: Ipv4Addr) -> Self;

    // Nicira extensions
    pub fn nx_reg_load(self, header: OxmHeader, value: u64) -> Self;
    pub fn nx_move(self, src: OxmHeader, dst: OxmHeader, n_bits: u16) -> Self;
    pub fn nx_learn(self, learn: NxLearn) -> Self;
    pub fn resubmit(self, port: Option<u16>, table: Option<u8>) -> Self;
    pub fn set_tunnel_id(self, id: u64) -> Self;
}
```

### NatConfig

```rust
pub struct NatConfig { ... }

impl NatConfig {
    // IPv4
    pub fn snat(addr: Ipv4Addr) -> Self;
    pub fn snat_range(min: Ipv4Addr, max: Ipv4Addr) -> Self;
    pub fn dnat(addr: Ipv4Addr) -> Self;
    pub fn dnat_range(min: Ipv4Addr, max: Ipv4Addr) -> Self;

    // IPv6
    pub fn snat_v6(addr: Ipv6Addr) -> Self;
    pub fn snat_v6_range(min: Ipv6Addr, max: Ipv6Addr) -> Self;
    pub fn dnat_v6(addr: Ipv6Addr) -> Self;
    pub fn dnat_v6_range(min: Ipv6Addr, max: Ipv6Addr) -> Self;

    // Options
    pub fn port(self, port: u16) -> Self;
    pub fn port_range(self, min: u16, max: u16) -> Self;
    pub fn random(self) -> Self;
    pub fn persistent(self) -> Self;
}
```

### PacketIn / PacketOut

```rust
pub struct PacketIn {
    pub buffer_id: u32,
    pub total_len: u16,
    pub reason: PacketInReason,
    pub table_id: u8,
    pub cookie: u64,
    pub in_port: u32,
    pub data: Vec<u8>,
}

pub struct PacketOut { ... }

impl PacketOut {
    pub fn new() -> Self;
    pub fn buffer_id(self, id: u32) -> Self;
    pub fn in_port(self, port: u32) -> Self;
    pub fn actions(self, actions: ActionList) -> Self;
    pub fn data(self, data: Vec<u8>) -> Self;
}
```

### Connection Tracking Constants

```rust
pub mod ct_flags {
    pub const COMMIT: u16 = 1 << 0;
    pub const FORCE: u16 = 1 << 1;
}

pub mod ct_state {
    pub const NEW: u32 = 1 << 0;
    pub const EST: u32 = 1 << 1;
    pub const REL: u32 = 1 << 2;
    pub const RPL: u32 = 1 << 3;
    pub const INV: u32 = 1 << 4;
    pub const TRK: u32 = 1 << 5;
    pub const SNAT: u32 = 1 << 6;
    pub const DNAT: u32 = 1 << 7;
}

pub const CT_COMMIT: u16 = ct_flags::COMMIT;
```

---

## rovs-ext

High-level abstractions for common OVS patterns.

### Flow Templates

#### SnatGateway

```rust
pub struct SnatConfig { ... }

impl SnatConfig {
    pub fn new(external_ip: Ipv4Addr, internal_port: u32, external_port: u32) -> Self;
    pub fn new_v6(external_ip: Ipv6Addr, internal_port: u32, external_port: u32) -> Self;
    pub fn dual_stack(v4: Ipv4Addr, v6: Ipv6Addr, internal: u32, external: u32) -> Self;
    pub fn zone(self, zone: u16) -> Self;
    pub fn port_range(self, min: u16, max: u16) -> Self;
    pub fn random(self) -> Self;
    pub fn ip_range(self, max: Ipv4Addr) -> Self;
    pub fn ip_v6_range(self, max: Ipv6Addr) -> Self;
}

pub struct SnatGateway { ... }

impl SnatGateway {
    pub fn new(config: SnatConfig) -> Self;
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, base_table: u8, priority: u16) -> Result<()>;
    pub async fn delete(&self, conn: &mut VConn, base_table: u8) -> Result<()>;
}
```

#### DnatService

```rust
pub struct DnatConfig { ... }

impl DnatConfig {
    pub fn new(external_port: u32, internal_port: u32) -> Self;
    pub fn zone(self, zone: u16) -> Self;
    pub fn add_rule(self, rule: DnatRule) -> Self;
    pub fn forward_tcp(self, ext_port: u16, int_ip: Ipv4Addr, int_port: u16) -> Self;
    pub fn forward_udp(self, ext_port: u16, int_ip: Ipv4Addr, int_port: u16) -> Self;
    pub fn forward_tcp_v6(self, ext_port: u16, int_ip: Ipv6Addr, int_port: u16) -> Self;
    pub fn forward_udp_v6(self, ext_port: u16, int_ip: Ipv6Addr, int_port: u16) -> Self;
}

pub struct DnatService { ... }

impl DnatService {
    pub fn new(config: DnatConfig) -> Self;
    pub fn all_flows(&self, base_table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, base_table: u8, priority: u16) -> Result<()>;
}
```

#### MacNatFlows

```rust
pub struct MacNatConfig {
    pub internal_mac: [u8; 6],
    pub external_mac: [u8; 6],
    pub internal_port: u32,
    pub external_port: u32,
}

pub struct MacNatFlows { ... }

impl MacNatFlows {
    pub fn new(config: MacNatConfig) -> Self;
    pub fn ipv4_outbound(&self, table: u8, priority: u16) -> Flow;
    pub fn ipv4_inbound(&self, table: u8, priority: u16) -> Flow;
    pub fn ipv6_outbound(&self, table: u8, priority: u16) -> Flow;
    pub fn ipv6_inbound(&self, table: u8, priority: u16) -> Flow;
    pub fn arp_outbound(&self, table: u8, priority: u16) -> Flow;
    pub fn arp_inbound(&self, table: u8, priority: u16) -> Flow;
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()>;
}
```

#### LearningSwitchFlows

```rust
pub struct LearningConfig { ... }

impl LearningConfig {
    pub fn new() -> Self;
    pub fn idle_timeout(self, secs: u16) -> Self;
    pub fn hard_timeout(self, secs: u16) -> Self;
    pub fn priority(self, priority: u16) -> Self;
    pub fn flood_ports(self, ports: Vec<u32>) -> Self;
}

pub struct LearningSwitchFlows { ... }

impl LearningSwitchFlows {
    pub fn new(config: LearningConfig) -> Self;
    pub fn learning_flow(&self, table: u8, priority: u16) -> Flow;
    pub fn flood_flow(&self, table: u8, priority: u16) -> Flow;
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()>;
}
```

#### ArpProxyFlows / NdpProxyFlows

```rust
pub struct ArpProxyFlows { ... }

impl ArpProxyFlows {
    pub fn builder() -> ArpProxyBuilder;
    pub fn all_flows(&self, table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()>;
}

pub struct NdpProxyFlows { ... }

impl NdpProxyFlows {
    pub fn builder() -> NdpProxyBuilder;
    pub fn to_controller_flows(&self, table: u8, priority: u16) -> Vec<Flow>;
    pub async fn install(&self, conn: &mut VConn, table: u8, priority: u16) -> Result<()>;
}
```

### Topology Builders

#### BridgePair

```rust
pub struct BridgePair { ... }

impl BridgePair {
    pub fn new(bridge1: impl Into<String>, bridge2: impl Into<String>) -> Self;
    pub fn vlans(self, vlans: Vec<u16>) -> Self;
    pub fn patch_names(self, name1: impl Into<String>, name2: impl Into<String>) -> Self;
    pub fn build_transaction(&self) -> Transaction;
    pub async fn create(&self, client: &mut Client) -> Result<()>;
}
```

#### VlanTrunk

```rust
pub struct VlanTrunk { ... }

impl VlanTrunk {
    pub fn new(bridge: impl Into<String>) -> Self;
    pub fn existing_bridge(self) -> Self;
    pub fn add_access_port(self, config: AccessPortConfig) -> Self;
    pub fn add_trunk_port(self, config: TrunkPortConfig) -> Self;
    pub fn build_transaction(&self) -> Transaction;
    pub async fn create(&self, client: &mut Client) -> Result<()>;
}

pub struct AccessPortConfig { ... }

impl AccessPortConfig {
    pub fn new(name: impl Into<String>, vlan: u16) -> Self;
    pub fn system(self) -> Self;  // Use existing system interface
}

pub struct TrunkPortConfig { ... }

impl TrunkPortConfig {
    pub fn new(name: impl Into<String>) -> Self;
    pub fn vlans(self, vlans: Vec<u16>) -> Self;
    pub fn all_vlans(self) -> Self;
}
```

### Controller Framework

```rust
pub struct Controller { ... }

impl Controller {
    pub async fn new(addr: &Address, config: ControllerConfig) -> Result<Self>;
    pub fn conn(&self) -> &VConn;
    pub fn conn_mut(&mut self) -> &mut VConn;
    pub fn register<H: PacketHandler + 'static>(&mut self, handler: H);
    pub async fn run(&mut self) -> Result<()>;
    pub async fn run_once(&mut self) -> Result<HandlerAction>;
}

pub struct ControllerConfig { ... }

impl ControllerConfig {
    pub fn new() -> Self;
    pub fn log_unhandled(self, log: bool) -> Self;
}

pub trait PacketHandler: Send + Sync {
    fn can_handle(&self, event: &PacketInEvent) -> bool;
    fn handle(&mut self, event: &PacketInEvent, conn: &mut VConn)
        -> impl Future<Output = Result<HandlerAction>> + Send;
}

pub enum HandlerAction {
    Handled,
    NotHandled,
    Stop,
}
```

### Protocol Handlers

```rust
// ARP Proxy Handler
pub struct ArpProxyHandler { ... }

impl ArpProxyHandler {
    pub fn new() -> Self;
    pub fn add_entry(&mut self, ip: [u8; 4], mac: [u8; 6]);
}

// NDP Proxy Handler
pub struct NdpProxyHandler { ... }

impl NdpProxyHandler {
    pub fn new() -> Self;
    pub fn add_entry(&mut self, ip: Ipv6Addr, mac: [u8; 6]);
}
```

### Utilities

```rust
// MAC/IP conversion
pub fn parse_mac(s: &str) -> Result<[u8; 6]>;
pub fn format_mac(mac: &[u8; 6]) -> String;
pub fn mac_to_u64(mac: &[u8; 6]) -> u64;

pub fn parse_ipv4(s: &str) -> Result<[u8; 4]>;
pub fn format_ipv4(ip: &[u8; 4]) -> String;
pub fn ipv4_to_u32(ip: &[u8; 4]) -> u32;

// Port mapping
pub struct PortMapper { ... }

impl PortMapper {
    pub fn new() -> Self;
    pub fn insert(&mut self, name: impl Into<String>, ofport: u32);
    pub fn get(&self, name: &str) -> Option<u32>;
    pub fn require(&self, name: &str) -> Result<u32>;
    pub fn remove_by_name(&mut self, name: &str) -> Option<u32>;
    pub fn remove_by_ofport(&mut self, ofport: u32) -> Option<String>;
}
```

---

## rovs-types

Shared types across crates.

```rust
pub use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub fn new(bytes: [u8; 6]) -> Self;
    pub fn from_slice(slice: &[u8]) -> Option<Self>;
    pub fn as_bytes(&self) -> &[u8; 6];
    pub fn is_broadcast(&self) -> bool;
    pub fn is_multicast(&self) -> bool;
}
```
