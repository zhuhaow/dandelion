use super::{device::Device, dns::FakeDns};
use crate::Result;
use anyhow::{bail, ensure};
use bytes::{Bytes, BytesMut};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use nix::sys::socket::SockAddr;
use pnet_packet::{
    ip::IpNextHeaderProtocols,
    ipv4::{checksum, Ipv4Packet, MutableIpv4Packet},
    udp::{ipv4_checksum, MutableUdpPacket, UdpPacket},
    MutablePacket, Packet,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio_util::codec::Framed;
use trust_dns_client::{
    op::Message,
    serialize::binary::{BinDecodable, BinEncodable, BinEncoder},
};
use tun::{TunPacket, TunPacketCodec};

pub async fn run_stack(
    device: Device,
    dns_server: FakeDns,
    fake_dns_server_addr: SocketAddr,
    mtu: usize,
) -> Result<()> {
    let (sink, stream) = device.into_framed(mtu).split();

    let stack_impl = StackImpl::new(
        Arc::new(Mutex::new(sink)),
        dns_server,
        fake_dns_server_addr,
        mtu,
    );

    stream
        .for_each_concurrent(10, |p| async {
            let result: Result<()> = async {
                let p = p?;
                stack_impl.input(p.get_bytes()).await?;
                Ok(())
            }
            .await;

            if let Err(err) = result {
                log::info!(
                    "Error happened when handing packets from TUN interface: {}",
                    err
                );
            }
        })
        .await;

    Ok(())
}

struct StackImpl {
    sink: Arc<Mutex<SplitSink<Framed<Device, TunPacketCodec>, TunPacket>>>,
    dns_server: FakeDns,
    fake_dns_server_addr: SocketAddr,
    mtu: usize,
}

impl StackImpl {
    fn new(
        sink: Arc<Mutex<SplitSink<Framed<Device, TunPacketCodec>, TunPacket>>>,
        dns_server: FakeDns,
        fake_dns_server_addr: SocketAddr,
        mtu: usize,
    ) -> Self {
        Self {
            sink,
            dns_server,
            fake_dns_server_addr,
            mtu,
        }
    }

    async fn input(&self, packet_buf: &[u8]) -> Result<()> {
        let packet = Ipv4Packet::new(packet_buf)
            .ok_or_else(|| anyhow::anyhow!("Not a valid Ipv4 packet"))?;

        if packet.get_next_level_protocol() == IpNextHeaderProtocols::Udp {
            let udp_packet = UdpPacket::new(packet.payload())
                .ok_or_else(|| anyhow::anyhow!("Not a valid UDP packet"))?;
            let addr: IpAddr = packet.get_destination().into();

            if SocketAddr::from((addr, udp_packet.get_destination())) == self.fake_dns_server_addr {
                self.handle_dns(&packet, &udp_packet).await?;
            }
        }

        Ok(())
    }

    async fn handle_dns<'a>(
        &self,
        inbound_packet: &'a Ipv4Packet<'a>,
        // There is an issue in the type def require the T to be a ref.
        inbound_udp_packet: &'a UdpPacket<'a>,
    ) -> Result<()> {
        let dns_request = Message::from_bytes(inbound_udp_packet.payload())?;
        let dns_response = self.dns_server.handle(dns_request).await?;
        let mut dns_response_buf = Vec::new();
        let mut encoder = BinEncoder::new(&mut dns_response_buf);
        dns_response.emit(&mut encoder)?;

        let totol_len = MutableIpv4Packet::minimum_packet_size()
            + MutableUdpPacket::minimum_packet_size()
            + dns_response_buf.len();

        ensure!(
            totol_len <= self.mtu,
            "Outbound packet is larger than MTU {} > {}",
            totol_len,
            self.mtu
        );

        let mut response = BytesMut::new();
        response.resize(totol_len, 0);

        let mut ipv4_packet = MutableIpv4Packet::new(response.as_mut()).unwrap();
        ipv4_packet.set_version(4);
        // We don't have any options.
        ipv4_packet.set_header_length(MutableIpv4Packet::minimum_packet_size() as u8 / 4);
        ipv4_packet.set_total_length(totol_len as u16);
        // Don't fragment.
        ipv4_packet.set_flags(0x10);
        ipv4_packet.set_ttl(64);
        ipv4_packet.set_next_level_protocol(IpNextHeaderProtocols::Udp);
        ipv4_packet.set_source(inbound_packet.get_destination());
        ipv4_packet.set_destination(inbound_packet.get_source());
        ipv4_packet.set_checksum(checksum(&ipv4_packet.to_immutable()));

        let mut udp_packet = MutableUdpPacket::new(ipv4_packet.payload_mut()).unwrap();
        udp_packet.set_source(inbound_udp_packet.get_destination());
        udp_packet.set_destination(inbound_udp_packet.get_source());
        udp_packet.set_length(
            MutableUdpPacket::minimum_packet_size() as u16 + dns_response_buf.len() as u16,
        );
        udp_packet.set_payload(&dns_response_buf);
        udp_packet.set_checksum(ipv4_checksum(
            &udp_packet.to_immutable(),
            &inbound_packet.get_destination(),
            &inbound_packet.get_source(),
        ));

        self.send(response.freeze()).await?;

        Ok(())
    }

    fn translate<'a>(&self, inbound_packet: &'a Ipv4Packet<'a>) -> Result<()> {
        // We only handle TCP for now.
        if inbound_packet.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
            bail!("Do not support translate packet other than TCP");
        }

        Ok(())
    }

    async fn send(&self, packet: Bytes) -> Result<()> {
        self.sink
            .lock()
            .await
            .send(TunPacket::new(packet.to_vec()))
            .await?;

        Ok(())
    }
}
