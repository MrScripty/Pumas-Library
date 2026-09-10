//! Positive listener attribution for the retained direct runtime child. This
//! inspects only its descriptor table and network namespace, never other PIDs.
use crate::models::RuntimeEndpointUrl;
use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

pub(crate) fn owns_listener(pid: u32, endpoint: &RuntimeEndpointUrl) -> io::Result<bool> {
    let url = url::Url::parse(endpoint.as_str()).map_err(io::Error::other)?;
    let address: IpAddr = url
        .host_str()
        .ok_or_else(|| io::Error::other("Runtime endpoint host absent"))?
        .trim_matches(['[', ']'])
        .parse()
        .map_err(io::Error::other)?;
    if !address.is_loopback() {
        return Err(io::Error::other(
            "Runtime listener attribution requires a loopback address",
        ));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| io::Error::other("Runtime endpoint port absent"))?;
    inspect(&Path::new("/proc").join(pid.to_string()), address, port)
}

fn inspect(process: &Path, address: IpAddr, port: u16) -> io::Result<bool> {
    let mut sockets = HashSet::new();
    for entry in std::fs::read_dir(process.join("fd"))? {
        let entry = entry?;
        let target = match std::fs::read_link(entry.path()) {
            Ok(target) => target,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let target = target.to_string_lossy();
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|s| s.strip_suffix(']'))
        {
            sockets.insert(inode.parse::<u64>().map_err(io::Error::other)?);
        }
    }
    let table_name = if address.is_ipv4() {
        "net/tcp"
    } else {
        "net/tcp6"
    };
    let table = std::fs::read_to_string(process.join(table_name))?;
    table_has_owned_listener(&table, address, port, &sockets)
}

fn table_has_owned_listener(
    table: &str,
    address: IpAddr,
    port: u16,
    sockets: &HashSet<u64>,
) -> io::Result<bool> {
    for line in table.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 10 {
            return Err(io::Error::other("Incomplete runtime TCP observation"));
        }
        if fields[3] != "0A" {
            continue;
        }
        let (ip, observed_port) = fields[1]
            .split_once(':')
            .ok_or_else(|| io::Error::other("Invalid runtime TCP endpoint"))?;
        let observed_port = u16::from_str_radix(observed_port, 16).map_err(io::Error::other)?;
        if observed_port != port {
            continue;
        }
        let observed_address = parse_address(ip)?;
        let inode = fields[9].parse::<u64>().map_err(io::Error::other)?;
        if observed_address == address && sockets.contains(&inode) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_address(text: &str) -> io::Result<IpAddr> {
    match text.len() {
        8 => Ok(Ipv4Addr::from(
            u32::from_str_radix(text, 16)
                .map_err(io::Error::other)?
                .to_ne_bytes(),
        )
        .into()),
        32 => {
            let mut bytes = [0u8; 16];
            for (part, destination) in text
                .as_bytes()
                .chunks_exact(8)
                .zip(bytes.chunks_exact_mut(4))
            {
                let part = std::str::from_utf8(part).map_err(io::Error::other)?;
                destination.copy_from_slice(
                    &u32::from_str_radix(part, 16)
                        .map_err(io::Error::other)?
                        .to_ne_bytes(),
                );
            }
            Ok(Ipv6Addr::from(bytes).into())
        }
        _ => Err(io::Error::other("Invalid runtime TCP address width")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_listener_address_port_state_and_owned_inode_are_required() {
        let table = "header\n 0: 0100007F:9876 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 81234\n";
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(table_has_owned_listener(table, ip, 0x9876, &HashSet::from([81234])).unwrap());
        assert!(!table_has_owned_listener(table, ip, 0x9876, &HashSet::from([999])).unwrap());
        assert!(!table_has_owned_listener(table, ip, 1234, &HashSet::from([81234])).unwrap());
        assert!(!table_has_owned_listener(
            &table.replace("0100007F", "00000000"),
            ip,
            0x9876,
            &HashSet::from([81234])
        )
        .unwrap());
        assert!(!table_has_owned_listener(
            &table.replace(" 0A ", " 01 "),
            ip,
            0x9876,
            &HashSet::from([81234])
        )
        .unwrap());
        assert!(table_has_owned_listener("header\ninvalid", ip, 1, &HashSet::new()).is_err());
    }
}
