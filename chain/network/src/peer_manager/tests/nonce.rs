use crate::network_protocol::testonly as data;
use crate::network_protocol::{
    Encoding, Handshake, PartialEdgeInfo, PeerMessage, EDGE_MIN_TIMESTAMP_NONCE,
};
use crate::peer_manager;
use crate::tcp;
use crate::testonly::make_rng;
use crate::testonly::stream;
use crate::time;
use near_o11y::testonly::init_test_logger;
use near_primitives::network::PeerId;
use near_primitives::version;
use std::sync::Arc;

// Nonces must be odd (as even ones are reserved for tombstones).
fn to_active_nonce(timestamp: time::Utc) -> u64 {
    let value = timestamp.unix_timestamp() as u64;
    if value % 2 == 0 {
        value + 1
    } else {
        value
    }
}

// Test connecting to peer manager with timestamp-like nonces.
#[tokio::test]
async fn test_nonces() {
    init_test_logger();
    let mut rng = make_rng(921853233);
    let rng = &mut rng;
    let mut clock = time::FakeClock::new(*EDGE_MIN_TIMESTAMP_NONCE + time::Duration::days(2));
    let chain = Arc::new(data::Chain::make(&mut clock, rng, 10));

    let test_cases = [
        // Try to connect with peer with a valid nonce (current timestamp).
        (to_active_nonce(clock.now_utc()), true, "current timestamp"),
        // Now try the peer with invalid timestamp (in the past)
        (to_active_nonce(clock.now_utc() - time::Duration::days(1)), false, "past timestamp"),
        // Now try the peer with invalid timestamp (in the future)
        (to_active_nonce(clock.now_utc() + time::Duration::days(1)), false, "future timestamp"),
        (u64::MAX, false, "u64 max"),
        (i64::MAX as u64, false, "i64 max"),
        ((i64::MAX - 1) as u64, false, "i64 max - 1"),
        (253402300799, false, "Max time"),
        (253402300799 + 2, false, "Over max time"),
        //(Some(0), false, "Nonce 0"),
        (1, true, "Nonce 1"),
    ];

    for test in test_cases {
        tracing::info!(target: "test", "Running test {:?}", test.2);
        // Start a PeerManager and connect a peer to it.
        let pm = peer_manager::testonly::start(
            clock.clock(),
            near_store::db::TestDB::new(),
            chain.make_config(rng),
            chain.clone(),
        )
        .await;

        let stream = tcp::Stream::connect(&pm.peer_info()).await.unwrap();
        let mut stream = stream::Stream::new(Some(Encoding::Proto), stream);
        let peer_key = data::make_secret_key(rng);
        let peer_id = PeerId::new(peer_key.public_key());
        let handshake = PeerMessage::Handshake(Handshake {
            protocol_version: version::PROTOCOL_VERSION,
            oldest_supported_version: version::PEER_MIN_ALLOWED_PROTOCOL_VERSION,
            sender_peer_id: peer_id.clone(),
            target_peer_id: pm.cfg.node_id(),
            // we have to set this even if we have no intention of listening since otherwise
            // the peer will drop our connection
            sender_listen_port: Some(24567),
            sender_chain_info: chain.get_peer_chain_info(),
            partial_edge_info: PartialEdgeInfo::new(&peer_id, &pm.cfg.node_id(), test.0, &peer_key),
        });
        stream.write(&handshake).await;
        if test.1 {
            match stream.read().await {
                Ok(PeerMessage::Handshake { .. }) => {}
                got => panic!("got = {got:?}, want Handshake"),
            }
        } else {
            match stream.read().await {
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {}
                got => panic!("got = {got:?}, want UnexpectedEof"),
            }
        }
    }
}
