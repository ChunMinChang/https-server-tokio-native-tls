use argh::FromArgs;
use std::fs::File;
use std::io::{self, prelude::*};
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_native_tls::native_tls;
use tokio_native_tls::TlsAcceptor;

#[derive(FromArgs)]
#[argh(description = "HTTPS server settings")]
struct Options {
    #[argh(positional)]
    addr: String,

    #[argh(option, short = 'c')]
    #[argh(description = "the certificate file in pkcs12 format for the server")]
    pkcs12: PathBuf,

    #[argh(option, short = 'p')]
    #[argh(description = "the password for the pkcs12 file")]
    password: String,
}
#[tokio::main]
async fn main() -> io::Result<()> {
    let options: Options = argh::from_env();
    let addr = options
        .addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let identity = load_identity(&options.pkcs12, &options.password)?;

    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(identity)
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    );

    let tcp_listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, peer_addr) = tcp_listener.accept().await?;
        println!("Connection from: {}", peer_addr);
        let acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(acceptor, stream).await {
                eprintln!("Error: {:?}", err);
            }
        });
    }
}

async fn handle_connection(tls_acceptor: TlsAcceptor, tcp_stream: TcpStream) -> io::Result<()> {
    let handshake = tls_acceptor.accept(tcp_stream);
    let mut tls_stream = handshake
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
    tls_stream
        .write_all(
            &b"HTTP/1.0 200 ok\r\n\
        Connection: close\r\n\
        Content-length: 12\r\n\
        \r\n\
        Hello world!"[..],
        )
        .await?;
    tls_stream.shutdown().await?;
    Ok(())
}

fn load_identity(path: &Path, password: &str) -> io::Result<native_tls::Identity> {
    let mut file = File::open(path)?;
    let mut identity = vec![];
    file.read_to_end(&mut identity)?;
    native_tls::Identity::from_pkcs12(&identity, password)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}
