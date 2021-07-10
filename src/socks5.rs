use crate::common::Address;
use anyhow::{anyhow, Result};
use log::debug;
use socks5_protocol::{
    AuthMethod, AuthRequest, AuthResponse, CommandReply, CommandRequest, CommandResponse, Version,
};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

pub struct Socks5Listener {
    listener: TcpListener,
}

pub struct Socks5Stream {}

impl Socks5Listener {
    pub async fn bind<A>(addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        let listener = TcpListener::bind(addr).await?;
        Ok(Socks5Listener { listener })
    }
    #[inline]
    pub async fn accept<'a>(&self) -> Result<(TcpStream, Address)> {
        let (mut stream, addr) = self.listener.accept().await?;
        debug!("accepted {}", addr);

        //Auth
        let version = Version::read(&mut stream).await?;
        let request = AuthRequest::read(&mut stream).await?;
        if request.select_from(&[AuthMethod::Noauth]) == AuthMethod::NoAcceptableMethod {
            return Err(anyhow!("unkoown method"));
        }
        version.write(&mut stream).await?;
        let respond = AuthResponse::new(AuthMethod::Noauth);
        respond.write(&mut stream).await?;

        //Get target address
        let request = CommandRequest::read(&mut stream).await?;

        //Respond
        match request.command {
            socks5_protocol::Command::Connect => {
                let address = Address::from_socks5addr(&request.address);
                let respond = CommandResponse::success(request.address);
                respond.write(&mut stream).await?;
                Ok((stream, address))
            }

            socks5_protocol::Command::Bind => {
                let respond = CommandResponse::reply_error(
                    socks5_protocol::CommandReply::CommandNotSupported,
                );
                respond.write(&mut stream).await?;
                Err(anyhow!("bind command isn't supported"))
            }
            socks5_protocol::Command::UdpAssociate => {
                let respond = CommandResponse::reply_error(
                    socks5_protocol::CommandReply::CommandNotSupported,
                );
                respond.write(&mut stream).await?;
                Err(anyhow!("udp accociate command isn't supported"))
            }
        }
    }
}

impl Socks5Stream {
    #[inline]
    pub async fn connect(server: &SocketAddr, addr: &Address) -> Result<TcpStream> {
        let mut stream = TcpStream::connect(server).await?;

        //Auth
        let version = Version::V5;
        version.write(&mut stream).await?;
        let request = AuthRequest::new(vec![AuthMethod::Noauth]);
        request.write(&mut stream).await?;
        let _version = Version::read(&mut stream).await?;
        let respond = AuthResponse::read(&mut stream).await?;
        if respond.method() != AuthMethod::Noauth {
            return Err(anyhow!("no acceptable socks5 auth method"));
        }

        //Command request
        let request = CommandRequest::connect(addr.to_socks5addr());
        request.write(&mut stream).await?;
        let respond = CommandResponse::read(&mut stream).await?;
        if respond.reply != CommandReply::Succeeded {
            return Err(anyhow!("socks5 command request error: {:?}", respond.reply));
        }

        Ok(stream)
    }
}
