//! Measure lazy item access refresh after real, unrelated grant changes.
use super::*;
use calternal_types::perf_monotonic_ns;
use std::sync::atomic::{AtomicU64, Ordering};

const ROOMS: usize = 32;
const PEERS: usize = 31;
const SECONDS: usize = 5;
const INTERVAL: Duration = Duration::from_secs(1);

/// Peer apply latency during global item authority changes.
pub(crate) const METRIC: &str = "sync.round_trip.authority_storm";
/// One sample per peer, room and preceding authority change.
pub(crate) const SAMPLES: usize = ROOMS * PEERS * SECONDS;

/// Keep 1,024 authenticated memberships live. Reuse the fixture session to
/// isolate the cost of per-membership View refresh from enrollment and setup.
pub(crate) fn sample(http: &mut Http, space: &str) -> Result<Vec<f64>> {
    let (unrelated, _) = http.op("spaces.create", json!({"name":"Unrelated grant changes"}))?;
    let (note, _) = http.op(
        "notes.create",
        json!({"space_id":unrelated["space"]["id"],"title":"Grant probe","markdown":"Outside all rooms"}),
    )?;
    let item = json!({"kind":"notes","id":note["note"]["id"]});
    let (group, _) = http.op("groups.create", json!({"name":"Unrelated grant target"}))?;
    let group = group["group_id"].as_str().ok_or("missing grant audience")?;
    let mut rooms = Vec::with_capacity(ROOMS);
    for n in 0..ROOMS {
        let (note, _) = http.op(
            "notes.create",
            json!({"space_id":space,"title":format!("Access storm {n}"),"markdown":"Start"}),
        )?;
        let id = note["note"]["id"].as_str().ok_or("missing storm item")?;
        rooms.push(format!("{space}/notes/{id}"));
    }
    let addresses: Vec<_> = rooms
        .iter()
        .enumerate()
        .map(|(index, room)| format!("@{}/{room}", index + 1))
        .collect();
    let open_replica = || -> Result<(Socket, Vec<Doc>)> {
        let mut socket = open(http)?;
        let mut docs = Vec::with_capacity(ROOMS);
        for (index, (room, address)) in rooms.iter().zip(&addresses).enumerate() {
            join_room(&mut socket, room, index + 1)?;
            docs.push(initial(&mut socket, address)?);
        }
        Ok((socket, docs))
    };
    let peers = (0..PEERS)
        .map(|_| open_replica())
        .collect::<Result<Vec<_>>>()?;
    let (mut writer, authored) = open_replica()?;
    let starts: Vec<_> = (0..SECONDS * ROOMS).map(|_| AtomicU64::new(0)).collect();
    std::thread::scope(|scope| -> Result<Vec<f64>> {
        let peers = peers
            .into_iter()
            .map(|(socket, docs)| {
                let (starts, addresses) = (&starts, &addresses);
                scope.spawn(move || {
                    observe(socket, docs, addresses, starts).map_err(|error| error.to_string())
                })
            })
            .collect::<Vec<_>>();
        let mut tick = Instant::now();
        for cycle in 0..SECONDS {
            std::thread::sleep(tick.saturating_duration_since(Instant::now()));
            let permission = if cycle % 2 == 0 { "view" } else { "edit" };
            let (grant, _) = http.op(
                "sharing.grant",
                json!({"item":item,"audience":{"group_id":group},"permission":permission}),
            )?;
            if grant["permission"] != permission {
                return Err("the unrelated grant did not change".into());
            }
            tick += INTERVAL;
            let interval = tick.saturating_duration_since(Instant::now()) / ROOMS as u32;
            for (room, (address, doc)) in addresses.iter().zip(&authored).enumerate() {
                let deadline = tick - interval * (ROOMS - room) as u32;
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                let text = doc.get_or_insert_text("md");
                let update = {
                    let mut txn = doc.transact_mut();
                    let offset = text.len(&txn);
                    starts[cycle * ROOMS + room].store(perf_monotonic_ns(), Ordering::Release);
                    text.insert(&mut txn, offset, "+");
                    txn.encode_update_v1()
                };
                send(
                    &mut writer,
                    address,
                    Message::Sync(SyncMessage::Update(update)),
                )?;
            }
        }
        let mut samples = Vec::with_capacity(SAMPLES);
        for peer in peers {
            samples.extend(peer.join().map_err(|_| "room storm peer panicked")??);
        }
        Ok(samples)
    })
}

fn observe(
    mut socket: Socket,
    docs: Vec<Doc>,
    addresses: &[String],
    starts: &[AtomicU64],
) -> Result<Vec<f64>> {
    let mut cycles = [0; ROOMS];
    let mut samples = Vec::with_capacity(SECONDS * ROOMS);
    while samples.len() < SECONDS * ROOMS {
        let (address, message) = any_message(&mut socket)?;
        let Message::Sync(SyncMessage::Update(update)) = message else {
            continue;
        };
        let room = addresses
            .iter()
            .position(|expected| *expected == address)
            .ok_or("room storm received a stale membership")?;
        if cycles[room] >= SECONDS {
            return Err("room storm produced an unexpected duplicate update".into());
        }
        apply(&docs[room], &update)?;
        let end = perf_monotonic_ns();
        let start = starts[cycles[room] * ROOMS + room].load(Ordering::Acquire);
        if start == 0 || end < start {
            return Err("room storm update lacks a matching keystroke".into());
        }
        let doc = &docs[room];
        let text = doc.get_or_insert_text("md").get_string(&doc.transact());
        if text.bytes().rev().take_while(|byte| *byte == b'+').count() != cycles[room] + 1 {
            return Err("room storm update did not advance the peer replica".into());
        }
        samples.push((end - start) as f64 / 1_000_000.);
        cycles[room] += 1;
    }
    Ok(samples)
}
