// Based on https://github.com/tokio-rs/tokio/blob/master/examples/proxy.rs
//
// Copyright (c) 2022 Tokio Contributors and Marlin Contributors
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.


mod utils;
mod addr_info;

use clap::Parser;
use futures::FutureExt;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio_vsock::{VsockListener, VsockStream};
use std::net::{SocketAddr, IpAddr};

use std::error::Error;

/// Creates a vsock proxy for ip server.
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// vsock address of the proxy to be set up <cid:port>
    #[clap(short, long, value_parser)]
    vsock_addr: String,
}

#[tokio::main]
pub async fn vsock_to_ip(cid: u32, port: u32) -> Result<(), Box<dyn Error>> {
    let listen_addr = (cid, port);

    println!("Listening on: {:?}", listen_addr);

    let mut listener = VsockListener::bind(listen_addr.0, listen_addr.1).expect("listener failed");

    while let Ok((inbound, _)) = listener.accept().await {
        let transfer = transfer(inbound).map(|r| {
            if let Err(e) = r {
                println!("Failed to transfer; error={}", e);
            }
        });

        tokio::spawn(transfer);
    }

    Ok(())
}

async fn transfer(inbound: VsockStream) -> Result<(), Box<dyn Error>> {
    let (mut ri, mut wi) = io::split(inbound);

    // read ip and port
    let proxy_addr = SocketAddr::new(IpAddr::V4(ri.read_u32_le().await?.into()), ri.read_u16_le().await?);
    println!("Proxying to: {:?}", proxy_addr);

    let mut outbound = TcpStream::connect(proxy_addr).await?;

    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let x = utils::split_vsock(&cli.vsock_addr).expect("vsock address not valid");
    match x {
        Some((cid, port)) => {
            let x = vsock_to_ip(cid, port);
            println!("{:?}", x);
        }
        None => {}
    }
}
