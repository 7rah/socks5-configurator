use anyhow::Result;
use socks5_protocol::Address as Socks5Address;
use std::{fmt, net::SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug, Clone)]
pub enum Address {
    SocketAddr(SocketAddr),
    Domain(String, u16),
}

impl Address {
    #[inline]
    pub fn to_socks5addr(&self) -> Socks5Address {
        match self {
            Address::SocketAddr(addr) => Socks5Address::SocketAddr(*addr),
            Address::Domain(domain, port) => Socks5Address::Domain(domain.clone(), *port),
        }
    }

    #[inline]
    pub fn from_socks5addr(addr: &Socks5Address) -> Address {
        match addr {
            Socks5Address::SocketAddr(addr) => Address::SocketAddr(*addr),
            Socks5Address::Domain(domain, port) => Address::Domain(domain.clone(), *port),
        }
    }
}

impl fmt::Display for Address {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Address::SocketAddr(ref addr) => write!(f, "{}", addr),
            Address::Domain(ref addr, ref port) => write!(f, "{}:{}", addr, port),
        }
    }
}

pub async fn copy_tcp<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    r: &mut R,
    w: &mut W,
) -> Result<()> {
    let mut buf = [0u8; 16384 * 3];
    loop {
        let len = r.read(&mut buf).await?;
        if len == 0 {
            break;
        }
        w.write(&buf[..len]).await?;
        w.flush().await?;
    }
    Ok(())
}
