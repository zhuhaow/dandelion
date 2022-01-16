use std::sync::Arc;
use std::time::Duration;

use ipnetwork::Ipv4Network;
use log::{debug, warn};
use specht2_core::acceptor::Acceptor;
use specht2_core::connector::tcp::TcpConnector;
use specht2_core::connector::Connector;
use specht2_core::resolver::udp::UdpResolver;
use specht2_core::tun::listening_address_for_subnet;
use specht2_core::tun::{device::Device, stack::create_stack};
use specht2_core::Result;
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let device = Device::new("10.128.0.1/12".parse().unwrap())?;

    let ip_block: Ipv4Network = "10.128.0.1/12".parse().unwrap();

    let resolver = Arc::new(
        UdpResolver::new(
            "114.114.114.114:53".parse().unwrap(),
            Duration::from_secs(5),
        )
        .await?,
    );

    let stack = create_stack(device, ip_block, resolver.clone()).await?;

    tokio::spawn(stack.0);

    let listener = TcpListener::bind(listening_address_for_subnet(&ip_block)).await?;

    let acceptor = Arc::new(stack.1);

    debug!("Start listening for new connection");

    loop {
        let result = listener.accept().await?;
        debug!("Got a new connection");

        let accept = acceptor.clone();
        let connector = TcpConnector::new(resolver.clone());

        tokio::spawn(async move {
            let result = async move {
                debug!("Do handshake");
                let (endpoint, fut) = accept.do_handshake(result.0).await?;
                debug!("Handshake done. Connecting...");
                let mut remote = connector.connect(&endpoint).await?;
                debug!("Connected. Forwarding...");
                let mut local = fut.await?;
                copy_bidirectional(&mut local, &mut remote).await?;

                Ok::<_, anyhow::Error>(())
            }
            .await;

            if let Err(err) = result {
                warn!("Error happened when processing a connection: {}", err)
            } else {
                debug!("Successfully processed connection");
            }
        });
    }
}
