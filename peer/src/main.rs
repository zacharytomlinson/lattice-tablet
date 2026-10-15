use lattice_tablet_peer::{Client, Packet};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 4 || !matches!(args[1].as_str(), "status" | "links") {
        return Err(
            "usage: lattice-peer DEVICE status | lattice-peer DEVICE links [OWNER_HEX] [VIA_HEX]"
                .into(),
        );
    }
    let mut client = Client::open(&args[0])?;
    let status = client.status()?;
    let Packet::Ready {
        address,
        founder,
        channel,
        ..
    } = status
    else {
        return Err(format!("peer not ready: {status:?}").into());
    };
    println!("READY address={address:016x} founder={founder:016x} channel={channel}");
    if args[1] == "status" {
        return Ok(());
    }
    let parse = |s: &String| u64::from_str_radix(s.trim_start_matches("0x"), 16);
    let owner = args.get(2).map(parse).transpose()?.unwrap_or(address);
    let via = args.get(3).map(parse).transpose()?.unwrap_or(0);
    let mut after = 0;
    let mut revision = None;
    for _ in 0..128 {
        let response = client.read_links(owner, after, via)?;
        let Packet::Link {
            revision: current,
            peer,
            round_trip_ms,
            ..
        } = response
        else {
            return Err(format!("read failed: {response:?}").into());
        };
        if revision.is_some_and(|r| r != current) {
            return Err("links changed during enumeration; restart with a new read".into());
        }
        revision = Some(current);
        if peer == 0 {
            println!("END owner={owner:016x} revision={current}");
            return Ok(());
        }
        if peer <= after {
            return Err("link cursor did not advance".into());
        }
        println!(
            "LINK owner={owner:016x} peer={peer:016x} round_trip_ms={round_trip_ms} revision={current}"
        );
        after = peer;
    }
    Err("enumeration exceeded 128 entries".into())
}
