use super::dns::FakeDns;
use crate::{utils::expiring_hash::ExpiringHashMap, Result};
use anyhow::bail;
use bytes::{Bytes, BytesMut};
use pnet_packet::{
    ip::IpNextHeaderProtocols,
    ipv4::{checksum, Ipv4Packet, MutableIpv4Packet},
    tcp::{ipv4_checksum, MutableTcpPacket, TcpPacket},
    MutablePacket, Packet,
};
use rand::{prelude::SliceRandom, Rng};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    ops::Range,
    time::Duration,
};

// TODO: Support IPv6
pub struct Translator {
    listening_addr: SocketAddrV4,
    real_source_to_fake_map: ExpiringHashMap<(SocketAddrV4, SocketAddrV4), SocketAddrV4>,
    fake_to_real_source_map: ExpiringHashMap<SocketAddrV4, (SocketAddrV4, SocketAddrV4)>,
    fake_ips: Vec<Ipv4Addr>,
    fake_port_range: Range<u16>,
}

impl Translator {
    pub fn new(
        listening_addr: SocketAddrV4,
        ips: Vec<Ipv4Addr>,
        ports: Range<u16>,
        ip_ttl: Duration,
    ) -> Self {
        Self {
            listening_addr,
            real_source_to_fake_map: ExpiringHashMap::new(ip_ttl, true),
            fake_to_real_source_map: ExpiringHashMap::new(ip_ttl, true),
            fake_ips: ips,
            fake_port_range: ports,
        }
    }

    pub fn translate<'a>(
        &mut self,
        inbound_packet: &'a Ipv4Packet<'a>,
        _dns: &'a FakeDns,
    ) -> Result<Bytes> {
        // We don't check if the target is in the fake ip subnet since we won't
        // see them otherwise.
        //
        // We only handle TCP for now.
        if inbound_packet.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
            bail!("Do not support translate packet other than TCP");
        }

        let inbound_tcp = TcpPacket::new(inbound_packet.payload())
            .ok_or_else(|| anyhow::anyhow!("Invalid TCP packet"))?;

        // There are two cases, we have a packet send to fake ip from outside or
        // a packet send to fake ip from the listening port.
        //
        // Check by source.
        if SocketAddrV4::new(inbound_packet.get_source(), inbound_tcp.get_source())
            == self.listening_addr
        {
            if let Some((real_dest, real_source)) =
                self.fake_to_real_source_map.get(&SocketAddrV4::new(
                    inbound_packet.get_destination(),
                    inbound_tcp.get_destination(),
                ))
            {
                // Query it once to refresh the expiring time.
                let _ = self
                    .real_source_to_fake_map
                    .get(&(*real_dest, *real_source));

                let mut response = BytesMut::from(inbound_packet.packet());
                let mut outbound = MutableIpv4Packet::new(response.as_mut()).unwrap();
                update_packet(&mut outbound, real_source, real_dest);

                Ok(response.freeze())
            } else {
                bail!(
                    "Failed to find mapping for address {}",
                    SocketAddrV4::new(
                        inbound_packet.get_destination(),
                        inbound_tcp.get_destination(),
                    )
                );
            }
        } else {
            let real_source =
                SocketAddrV4::new(inbound_packet.get_source(), inbound_tcp.get_source());
            let real_dest = SocketAddrV4::new(
                inbound_packet.get_destination(),
                inbound_tcp.get_destination(),
            );

            let fake_source = match self.real_source_to_fake_map.get(&(real_source, real_dest)) {
                Some(fake_source) => {
                    let _ = self.fake_to_real_source_map.get(fake_source);

                    *fake_source
                }
                None => loop {
                    // Clean up when we accept a new connection. This won't
                    // happen too much so it's ok. And we don't have to set up
                    // a timer for this.
                    self.clear_expired();
                    let mut rng = rand::thread_rng();
                    let ip = self.fake_ips.choose(&mut rng).unwrap();
                    let port = rng.gen_range(self.fake_port_range.clone());
                    let addr = SocketAddrV4::new(*ip, port);
                    if self.fake_to_real_source_map.get(&addr).is_none() {
                        self.fake_to_real_source_map
                            .insert(addr, (real_source, real_dest));
                        self.real_source_to_fake_map
                            .insert((real_source, real_dest), addr);
                        break addr;
                    }
                },
            };

            let mut response = BytesMut::from(inbound_packet.packet());
            let mut outbound = MutableIpv4Packet::new(response.as_mut()).unwrap();
            update_packet(&mut outbound, &fake_source, &self.listening_addr);

            Ok(response.freeze())
        }
    }

    pub fn look_up_source(&mut self, addr: &SocketAddrV4) -> Option<SocketAddrV4> {
        self.fake_to_real_source_map.get(addr).map(|p| p.1.clone())
    }

    fn clear_expired(&mut self) {
        self.fake_to_real_source_map.clear_expired();
        self.real_source_to_fake_map.clear_expired();
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
