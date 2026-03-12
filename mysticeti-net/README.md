# mysticeti-net

Iroh-based transport layer for Mysticeti.

- **Codec**: Length-delimited (4-byte len + bincode) encode/decode for `NetworkMessage`.
- **Protocol**: `MysticetiProtocol` implements `iroh::protocol::ProtocolHandler`; one bi stream per connection, message loop.
- **Peer identity**: Map Iroh `EndpointId` to committee `AuthorityIndex` via `PeerIdentity`.
- **Sync**: Helpers for subscribe/request/response (e.g. `Syncer::handle_request_blocks`).

ALPN: `mysticeti/0`.
