use std::io;

use std::num::NonZeroU32;
use std::os::unix::io::{AsRawFd, RawFd};

use std::io::Read;

use mio::unix::SourceFd;
use mio::{event::Source, Interest, Registry, Token};
use socket2::{Domain, Protocol, SockAddr, Socket as Socket2, Type};

pub struct Socket {
    socket: Socket2,
}

impl Socket {
    pub fn new(
        domain: Domain,
        type_: Type,
        protocol: Protocol,
        interface: Option<NonZeroU32>,
    ) -> io::Result<Self> {
        let socket = Socket2::new(domain, type_, Some(protocol))?;
        if domain == Domain::IPV4 {
            socket.bind_device_by_index_v4(interface)?;
        }
        if domain == Domain::IPV6 {
            socket.bind_device_by_index_v6(interface)?;
        }
        socket.set_nonblocking(true)?;

        Ok(Self { socket })
    }

    pub fn send_to(&self, buf: &[u8], target: &SockAddr) -> io::Result<usize> {
        self.socket.send_to(buf, target)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.socket).read(buf)
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

impl Source for Socket {
    fn register(&mut self, poll: &Registry, token: Token, interest: Interest) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(poll, token, interest)
    }

    fn reregister(&mut self, poll: &Registry, token: Token, interest: Interest) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(poll, token, interest)
    }

    fn deregister(&mut self, poll: &Registry) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(poll)
    }
}
