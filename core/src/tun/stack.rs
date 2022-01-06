use super::{device::Device, dns::TunDns};
use crate::Result;
use bytes::{Bytes, BytesMut};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use smoltcp::{
    phy::ChecksumCapabilities,
    wire::{IpAddress, IpProtocol, Ipv4Packet, Ipv4Repr, UdpPacket, UdpRepr},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
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
    dns_server: TunDns,
    fake_dns_server_addr: SocketAddr,
) -> Result<()> {
    let (sink, stream) = device.into_framed().split();

    let stack_impl = StackImpl::new(Arc::new(Mutex::new(sink)), dns_server, fake_dns_server_addr);

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
    dns_server: TunDns,
    fake_dns_server_addr: SocketAddr,
}

impl StackImpl {
    fn new(
        sink: Arc<Mutex<SplitSink<Framed<Device, TunPacketCodec>, TunPacket>>>,
        dns_server: TunDns,
        fake_dns_server_addr: SocketAddr,
    ) -> Self {
        Self {
            sink,
            dns_server,
            fake_dns_server_addr,
        }
    }

    async fn input(&self, packet_buf: &[u8]) -> Result<()> {
        // Assume there is no packet info
        let packet = Ipv4Packet::new_checked(packet_buf)?;

        if packet.protocol() == IpProtocol::Udp {
            let udp_packet = UdpPacket::new_checked(packet.payload())?;
            let addr: IpAddr = Ipv4Addr::from(packet.dst_addr()).into();
            if SocketAddr::from((addr, udp_packet.dst_port())) == self.fake_dns_server_addr {
                self.handle_dns(packet, udp_packet).await?;
            }
        }

        Ok(())
    }

    async fn handle_dns<'a>(
        &self,
        packet: Ipv4Packet<impl AsRef<[u8]> + 'a>,
        // There is an issue in the type def require the T to be a ref.
        udp_packet: UdpPacket<&'a [u8]>,
    ) -> Result<()> {
        let dns_request = Message::from_bytes(udp_packet.payload())?;
        let dns_response = self.dns_server.handle(dns_request).await?;
        let mut dns_response_buf = Vec::new();
        let mut encoder = BinEncoder::new(&mut dns_response_buf);
        dns_response.emit(&mut encoder)?;

        let udp_repr = UdpRepr {
            src_port: udp_packet.dst_port(),
            dst_port: udp_packet.src_port(),
        };

        let ip_repr = Ipv4Repr {
            src_addr: packet.dst_addr(),
            dst_addr: packet.src_addr(),
            protocol: IpProtocol::Udp,
            payload_len: udp_repr.header_len() + dns_response_buf.len(),
            hop_limit: 64,
        };

        let mut response_buf = BytesMut::new();
        response_buf.resize(ip_repr.buffer_len() + ip_repr.payload_len, 0);

        let mut ip_packet_response = Ipv4Packet::new_unchecked(&mut response_buf);
        ip_repr.emit(&mut ip_packet_response, &ChecksumCapabilities::default());

        let mut udp_packet_response = UdpPacket::new_unchecked(ip_packet_response.payload_mut());
        udp_repr.emit(
            &mut udp_packet_response,
            &IpAddress::Ipv4(packet.dst_addr()),
            &IpAddress::Ipv4(packet.src_addr()),
            dns_response_buf.len(),
            |buf| {
                buf.copy_from_slice(&dns_response_buf);
            },
            &ChecksumCapabilities::default(),
        );

        self.send(response_buf.freeze()).await?;

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
