use super::{
    device::{Device, DeviceWriter},
    dns::FakeDns,
    translator::Translator,
};
use crate::{
    acceptor::Acceptor,
    resolver::Resolver,
    tun::{acceptor::TunAcceptor, listening_address_for_subnet},
    Result,
};
use anyhow::ensure;
use bytes::{Bytes, BytesMut};
use futures::{Future, StreamExt};
use ipnetwork::Ipv4Network;
use pnet_packet::{
    ip::IpNextHeaderProtocols,
    ipv4::{checksum, Ipv4Packet, MutableIpv4Packet},
    udp::{ipv4_checksum, MutableUdpPacket, UdpPacket},
    MutablePacket, Packet,
};
use std::{net::SocketAddrV4, ops::Range, sync::Arc, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpStream, sync::Mutex};
use tracing::info;

use trust_dns_client::{
    op::Message,
    serialize::binary::{BinDecodable, BinEncodable, BinEncoder},
};

pub async fn create_stack<R: Resolver>(
    mut device: Device,
    subnet: Ipv4Network,
    resolver: R,
) -> Result<(impl Future<Output = ()>, impl Acceptor<TcpStream>)> {
    // It's easy to make them configurable but we don't need it yet.
    static MTU: usize = 1500;
    static DNS_TTL: Duration = Duration::from_secs(120);
    static FAKE_SNAT_IP_POOL_SIZE: usize = 10;
    static FAKE_SNAT_PORT_RANGE: Range<u16> = 1024..65535;
    static DNS_PORT: u16 = 53;

    ensure!(
        subnet.size() >= 2 ^ 16,
        "Subnet is too small. The tun needs a block at least /16."
    );

    let dns_ip = subnet.ip();
    let listening_addr = listening_address_for_subnet(&subnet);

    let mut iter = subnet.into_iter();

    (&mut iter).take_while(|ip| ip != &dns_ip).for_each(drop);

    assert!(iter.next().is_some());

    let fake_snap_ip_pool = (&mut iter).take(FAKE_SNAT_IP_POOL_SIZE).collect::<_>();

    let dns_server = Arc::new(FakeDns::new(resolver, iter, DNS_TTL).await?);

    let translator = Arc::new(Mutex::new(Translator::new(
        listening_addr,
        fake_snap_ip_pool,
        FAKE_SNAT_PORT_RANGE.clone(),
    )));

    let dns_server_clone = dns_server.clone();
    let translator_clone = translator.clone();

    info!("Tun DNS listening on {}", dns_ip);

    let packet_fut = async move {
        let writer = device.get_writer();

        let stack_impl = StackImpl::new(
            writer,
            dns_server_clone,
            SocketAddrV4::new(dns_ip, DNS_PORT),
            translator_clone,
            MTU,
        );

        async_stream::stream! {
            loop {
                let mut byte = BytesMut::new();
                byte.resize(MTU + 4, 0);
                let len = match device.read(byte.as_mut()).await {
                    Ok(len) => len,
                    Err(err) => {
                        yield Err(err);
                        break;
                    }
                };
                byte.truncate(len);
                yield Ok(byte);
            }
        }
        .for_each_concurrent(None, |p| async {
            let result: Result<()> = async {
                let p = p?;
                stack_impl.input(p).await?;
                Ok(())
            }
            .await;

            if let Err(err) = result {
                info!(
                    "Error happened when handing packets from TUN interface: {}",
                    err
                );
            }
        })
        .await
    };

    let acceptor = TunAcceptor::new(dns_server, translator);

    Ok((packet_fut, acceptor))
}

struct StackImpl<R: Resolver> {
    writer: DeviceWriter,
    dns_server: Arc<FakeDns<R>>,
    fake_dns_server_addr: SocketAddrV4,
    translator: Arc<Mutex<Translator>>,
    mtu: usize,
}

impl<R: Resolver> StackImpl<R> {
    fn new(
        writer: DeviceWriter,
        dns_server: Arc<FakeDns<R>>,
        fake_dns_server_addr: SocketAddrV4,
        translator: Arc<Mutex<Translator>>,
        mtu: usize,
    ) -> Self {
        Self {
            writer,
            dns_server,
            fake_dns_server_addr,
            translator,
            mtu,
        }
    }

    async fn input(&self, mut tun_buf: BytesMut) -> Result<()> {
        let packet_buf = &mut tun_buf[4..];

        let mut packet = MutableIpv4Packet::new(packet_buf)
            .ok_or_else(|| anyhow::anyhow!("Not a valid Ipv4 packet"))?;

        if packet.get_next_level_protocol() == IpNextHeaderProtocols::Udp {
            let udp_packet = UdpPacket::new(packet.payload())
                .ok_or_else(|| anyhow::anyhow!("Not a valid UDP packet"))?;

            if SocketAddrV4::new(packet.get_destination(), udp_packet.get_destination())
                == self.fake_dns_server_addr
            {
                self.handle_dns(&packet.to_immutable(), &udp_packet).await?;
            }
        } else {
            self.translate(&mut packet).await?;
            self.send(&tun_buf.freeze())?;
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
        response.resize(totol_len + 4, 0);

        response[3] = libc::PF_INET as u8;

        let response_buf = &mut response[4..];
        let mut ipv4_packet = MutableIpv4Packet::new(response_buf).unwrap();
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

        self.send(&response.freeze())?;

        Ok(())
    }

    async fn translate<'a>(&self, inbound_packet: &'a mut MutableIpv4Packet<'a>) -> Result<()> {
        self.translator.lock().await.translate(inbound_packet)
    }

    fn send(&self, packet: &Bytes) -> Result<()> {
        self.writer.write(packet.as_ref())
    }
}
