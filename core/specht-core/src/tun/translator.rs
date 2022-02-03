use crate::Result;
use anyhow::{bail, ensure};
use multi_map::MultiMap;
use pnet_packet::{
    ip::IpNextHeaderProtocols,
    ipv4::{checksum, MutableIpv4Packet},
    tcp::{
        ipv4_checksum, MutableTcpPacket,
        TcpFlags::{ACK, FIN, RST, SYN},
        TcpPacket,
    },
    MutablePacket, Packet,
};
use rand::{prelude::SliceRandom, Rng};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    ops::Range,
};
use tracing::{info, trace};

#[derive(PartialEq)]
enum State {
    On,
    GetFin { expect_ack: u32 },
    Closed,
}

struct Connection {
    fake_ip_state: State,
    real_pair_state: State,
    fake_ip: SocketAddrV4,
    real_pair: (SocketAddrV4, SocketAddrV4),
}

impl Connection {
    fn new(fake_ip: SocketAddrV4, real_pair: (SocketAddrV4, SocketAddrV4)) -> Self {
        Self {
            fake_ip,
            real_pair,
            fake_ip_state: State::On,
            real_pair_state: State::On,
        }
    }
}

type Key = SocketAddrV4;

struct ConnectionManager {
    map: MultiMap<SocketAddrV4, (SocketAddrV4, SocketAddrV4), Connection>,
    fake_ips: Vec<Ipv4Addr>,
    fake_port_range: Range<u16>,
    listening_addr: SocketAddrV4,
}

struct RewriteTo(SocketAddrV4, SocketAddrV4);

impl ConnectionManager {
    fn new(
        fake_ips: Vec<Ipv4Addr>,
        fake_port_range: Range<u16>,
        listening_addr: SocketAddrV4,
    ) -> Self {
        Self {
            map: Default::default(),
            fake_ips,
            fake_port_range,
            listening_addr,
        }
    }

    fn get_init_syn(&mut self, source: SocketAddrV4, target: SocketAddrV4) -> Result<RewriteTo> {
        ensure!(
            target != self.listening_addr,
            "Unexpected SYN send to {}",
            self.listening_addr,
        );

        // Check if this is a dup SYN
        match self.map.get_alt(&(source, target)) {
            Some(c) => Ok(RewriteTo(c.fake_ip, self.listening_addr)),
            None => {
                let fake_ip = self.get_fake_ip()?;
                trace!("Create new connection {} -> {}", source, target);
                let connection = Connection::new(fake_ip, (source, target));
                self.map.insert(fake_ip, (source, target), connection);
                Ok(RewriteTo(fake_ip, self.listening_addr))
            }
        }
    }

    fn get_rst(&mut self, source: SocketAddrV4, target: SocketAddrV4) -> Result<Option<RewriteTo>> {
        // we are not implementing RFC5963 to check the validity of RST since we
        // don't have any attack surface with internal only stack.
        match self.find_mapping(&source, &target) {
            Some((rewrite, key)) => {
                self.remove_connection(&key);

                Ok(Some(rewrite))
            }
            // Resetting nothing, ignore.
            None => Ok(None),
        }
    }

    fn get_packet(
        &mut self,
        source: SocketAddrV4,
        target: SocketAddrV4,
        ack: Option<u32>,
        seq: u32,
        fin: bool,
    ) -> Result<RewriteTo> {
        match self.find_mapping(&source, &target) {
            Some((rewrite, k)) => {
                let conn = self.map.get_mut(&k).unwrap();

                let (source_state, desitination_state) = if target == conn.fake_ip {
                    (&mut conn.real_pair_state, &mut conn.fake_ip_state)
                } else {
                    (&mut conn.fake_ip_state, &mut conn.real_pair_state)
                };

                if let State::On = desitination_state {
                    if fin {
                        *desitination_state = State::GetFin {
                            expect_ack: seq.saturating_add(1),
                        };
                    }
                };

                if let State::GetFin { expect_ack } = source_state {
                    if Some(expect_ack.to_owned()) == ack {
                        *source_state = State::Closed
                    }
                };

                if source_state == &State::Closed && desitination_state == &State::Closed {
                    trace!(
                        "Tun translator removed connection {} <-> {}",
                        source,
                        target
                    );
                    self.remove_connection(&k);
                }

                Ok(rewrite)
            }
            None => bail!("Failed to find source/destination mapping for packet",),
        }
    }

    fn remove_connection(&mut self, key: &Key) {
        self.map.remove(key);
    }

    fn get_fake_ip(&self) -> Result<SocketAddrV4> {
        for _ in 0..1000 {
            let mut rng = rand::thread_rng();
            let ip = self.fake_ips.choose(&mut rng).unwrap();
            let port = rng.gen_range(self.fake_port_range.clone());
            let addr = SocketAddrV4::new(*ip, port);
            if self.map.get(&addr).is_none() {
                return Ok(addr);
            }
        }

        bail!("Failed to generated a fake ip");
    }

    fn find_mapping(
        &mut self,
        source: &SocketAddrV4,
        target: &SocketAddrV4,
    ) -> Option<(RewriteTo, Key)> {
        if source == &self.listening_addr {
            self.map
                .get(target)
                .map(|c| (RewriteTo(c.real_pair.1, c.real_pair.0), c.fake_ip))
        } else {
            self.map
                .get_alt(&(*source, *target))
                .map(|c| (RewriteTo(c.fake_ip, self.listening_addr), c.fake_ip))
        }
    }
}

// TODO: Support IPv6
pub struct Translator {
    manager: ConnectionManager,
}

impl Translator {
    pub fn new(listening_addr: SocketAddrV4, ips: Vec<Ipv4Addr>, ports: Range<u16>) -> Self {
        Self {
            manager: ConnectionManager::new(ips, ports, listening_addr),
        }
    }

    pub fn translate<'a>(&mut self, inbound_packet: &'a mut MutableIpv4Packet<'a>) -> Result<()> {
        // We don't check if the target is in the fake ip subnet since we won't
        // see them otherwise.
        //
        // We only handle TCP for now.
        if inbound_packet.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
            bail!("Do not support translate packet other than TCP");
        }

        let inbound_tcp = TcpPacket::new(inbound_packet.payload())
            .ok_or_else(|| anyhow::anyhow!("Invalid TCP packet"))?;

        let source = SocketAddrV4::new(inbound_packet.get_source(), inbound_tcp.get_source());
        let target = SocketAddrV4::new(
            inbound_packet.get_destination(),
            inbound_tcp.get_destination(),
        );

        let result = if inbound_tcp.get_flags() == SYN {
            trace!("Get a SYN packet, {} -> {}", source, target);
            self.manager.get_init_syn(source, target)
        } else if inbound_tcp.get_flags() & RST != 0 {
            trace!("Get a RST packet, {} -> {}", source, target);
            match self.manager.get_rst(source, target) {
                Ok(result) => match result {
                    Some(rewrite) => Ok(rewrite),
                    None => bail!("Failed to translate the RST packet"),
                },
                Err(e) => Err(e),
            }
        } else {
            self.manager.get_packet(
                source,
                target,
                if inbound_tcp.get_flags() & ACK != 0 {
                    Some(inbound_tcp.get_acknowledgement())
                } else {
                    None
                },
                inbound_tcp.get_sequence(),
                inbound_tcp.get_flags() & FIN != 0,
            )
        };

        match result {
            Ok(rewrite) => update_packet(inbound_packet, &rewrite.0, &rewrite.1),
            Err(err) => {
                info!(
                    "Error happened when translating packet {} -> {}, {},  sending RST back.",
                    err, source, target
                );

                update_packet_to_rst(inbound_packet);
            }
        }

        Ok(())
    }

    pub fn look_up_source(&self, addr: &SocketAddrV4) -> Option<SocketAddrV4> {
        self.manager.map.get(addr).map(|c| c.real_pair.1)
    }
}

fn update_packet(packet: &mut MutableIpv4Packet, source: &SocketAddrV4, dest: &SocketAddrV4) {
    packet.set_source(*source.ip());
    packet.set_destination(*dest.ip());
    packet.set_checksum(checksum(&packet.to_immutable()));

    let mut packet_tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
    packet_tcp.set_source(source.port());
    packet_tcp.set_destination(dest.port());
    packet_tcp.set_checksum(ipv4_checksum(
        &packet_tcp.to_immutable(),
        source.ip(),
        dest.ip(),
    ));
}

fn update_packet_to_rst(packet: &mut MutableIpv4Packet) {
    let source_ip = packet.get_destination();
    let destination_ip = packet.get_source();

    packet.set_source(destination_ip);
    packet.set_destination(source_ip);

    let mut packet_tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
    let source_port = packet_tcp.get_destination();
    let destination_port = packet_tcp.get_source();
    let original_payload_length = packet_tcp.payload().len();

    packet_tcp.set_source(destination_port);
    packet_tcp.set_destination(source_port);
    packet_tcp.set_sequence(packet_tcp.get_acknowledgement());
    packet_tcp.set_flags(RST);
    packet_tcp.set_payload(&[]);
    packet_tcp.set_checksum(ipv4_checksum(
        &packet_tcp.to_immutable(),
        &source_ip,
        &destination_ip,
    ));

    packet.set_total_length(packet.get_total_length() - original_payload_length as u16);
    packet.set_checksum(checksum(&packet.to_immutable()));
}
