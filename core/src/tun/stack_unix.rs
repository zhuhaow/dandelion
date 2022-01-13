use super::{
    codec::{TunPacket, TunPacketCodec},
    device::Device,
    dns::FakeDns,
    translator::Translator,
};
use crate::{acceptor::Acceptor, resolver::Resolver, tun::acceptor::TunAcceptor, Result};
use anyhow::ensure;
use bytes::{Bytes, BytesMut};
use futures::{stream::SplitSink, Future, SinkExt, StreamExt};
use ipnetwork::Ipv4Network;
use log::debug;
use pnet_packet::{
    ip::IpNextHeaderProtocols,
    ipv4::{checksum, Ipv4Packet, MutableIpv4Packet},
    udp::{ipv4_checksum, MutableUdpPacket, UdpPacket},
    MutablePacket, Packet,
};
use std::{net::SocketAddrV4, ops::Range, sync::Arc, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
};
use tokio_stream::wrappers::ReceiverStream;
use trust_dns_client::{
    op::Message,
    serialize::binary::{BinDecodable, BinEncodable, BinEncoder},
};

pub async fn create_stack<R: Resolver>(
    device: Device,
    subnet: Ipv4Network,
    resolver: R,
    listening_addr: SocketAddrV4,
) -> Result<(impl Future<Output = ()>, impl Acceptor<TcpStream>)> {
    // It's easy to make them configurable but we don't need it yet.
    static MTU: usize = 1500;
    static DNS_TTL: Duration = Duration::from_secs(10);
    static IP_TTL: Duration = Duration::from_secs(180);
    static FAKE_SNAT_IP_POOL_SIZE: usize = 10;
    static FAKE_SNAT_PORT_RANGE: Range<u16> = 1024..65535;
    static DNS_PORT: u16 = 53;

    ensure!(
        subnet.size() >= 2 ^ 16,
        "Subnet is too small. The tun needs a block at least /16."
    );

    let dns_ip = subnet.ip();

    let mut iter = subnet.into_iter();

    (&mut iter).take_while(|ip| ip != &dns_ip).for_each(drop);

    assert!(iter.next().is_some());

    let fake_snap_ip_pool = (&mut iter).take(FAKE_SNAT_IP_POOL_SIZE).collect::<_>();

    let dns_server = Arc::new(FakeDns::new(resolver, iter, DNS_TTL).await?);

    let translator = Arc::new(Mutex::new(Translator::new(
        listening_addr,
        fake_snap_ip_pool,
        FAKE_SNAT_PORT_RANGE.clone(),
        IP_TTL,
    )));

    let (mut sink, stream) = device.into_framed(MTU).split();

    let dns_server_clone = dns_server.clone();
    let translator_clone = translator.clone();

    debug!("DNS listening on {}", dns_ip);

    let (tx, mut rx) = channel(100);
    // The task finishes when tx is dropped.
    tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            sink.send(packet).await;
        }
    });

    let packet_fut = async move {
        let stack_impl = StackImpl::new(
            tx,
            dns_server_clone,
            SocketAddrV4::new(dns_ip, DNS_PORT),
            translator_clone,
            MTU,
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
            .await
    };

    let acceptor = TunAcceptor::new(dns_server, translator);

    Ok((packet_fut, acceptor))
}

struct StackImpl<R: Resolver> {
    sender: Sender<TunPacket>,
    dns_server: Arc<FakeDns<R>>,
    fake_dns_server_addr: SocketAddrV4,
    translator: Arc<Mutex<Translator>>,
    mtu: usize,
}

impl<R: Resolver> StackImpl<R> {
    fn new(
        sender: Sender<TunPacket>,
        dns_server: Arc<FakeDns<R>>,
        fake_dns_server_addr: SocketAddrV4,
        translator: Arc<Mutex<Translator>>,
        mtu: usize,
    ) -> Self {
        Self {
            sender,
            dns_server,
            fake_dns_server_addr,
            translator,
            mtu,
        }
    }

    async fn input(&self, packet_buf: &[u8]) -> Result<()> {
        debug!("Got new packet input");

        let packet = Ipv4Packet::new(packet_buf)
            .ok_or_else(|| anyhow::anyhow!("Not a valid Ipv4 packet"))?;

        if packet.get_next_level_protocol() == IpNextHeaderProtocols::Udp {
            let udp_packet = UdpPacket::new(packet.payload())
                .ok_or_else(|| anyhow::anyhow!("Not a valid UDP packet"))?;

            if SocketAddrV4::new(packet.get_destination(), udp_packet.get_destination())
                == self.fake_dns_server_addr
            {
                self.handle_dns(&packet, &udp_packet).await?;
            }
        } else {
            self.send(self.translate(&packet).await?).await?;
        }

        Ok(())
    }

    async fn handle_dns<'a>(
        &self,
        inbound_packet: &'a Ipv4Packet<'a>,
        // There is an issue in the type def require the T to be a ref.
        inbound_udp_packet: &'a UdpPacket<'a>,
    ) -> Result<()> {
        debug!("Got a dns request");
        let dns_request = Message::from_bytes(inbound_udp_packet.payload())?;
        let dns_response = self.dns_server.handle(dns_request).await?;
        debug!("Got response for fake dns server");
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

    async fn translate<'a>(&self, inbound_packet: &'a Ipv4Packet<'a>) -> Result<Bytes> {
        let packet = self.translator.lock().await.translate(inbound_packet)?;

        Ok(packet)
    }

    async fn send(&self, packet: Bytes) -> Result<()> {
        debug!("Writing packet to tun");

        self.sender.send(TunPacket::new(packet.to_vec())).await?;

        Ok(())
    }
}
