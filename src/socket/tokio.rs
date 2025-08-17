use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use socket2::{Domain, Protocol, SockAddr, Socket as Socket2, Type};
use std::future::Future;
use std::io::Read;
use std::net::SocketAddr;
use tokio::io::unix::AsyncFd;

#[derive(Clone)]
pub struct Socket {
    socket: Arc<AsyncFd<Socket2>>,
}

impl Socket {
    pub fn new(domain: Domain, type_: Type, protocol: Protocol) -> io::Result<Self> {
        let socket = Socket2::new(domain, type_, Some(protocol))?;
        socket.set_nonblocking(true)?;
        let socket = AsyncFd::new(socket)?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    pub fn send_to<T>(&self, buf: T, target: &SocketAddr) -> Send<T>
    where
        T: AsRef<[u8]>,
    {
        Send {
            state: SendState::Writing {
                socket: self.socket.clone(),
                addr: (*target).into(),
                buf,
            },
        }
    }

    pub fn recv(&self, buffer: &mut [u8], cx: &mut Context<'_>) -> Poll<Result<usize, io::Error>> {
        loop {
            match self.socket.poll_read_ready(cx) {
                Poll::Ready(Ok(mut guard)) => match guard.try_io(|fd| fd.get_ref().read(buffer)) {
                    Ok(res) => return Poll::Ready(res),
                    Err(_) => continue,
                },
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub struct Send<T> {
    state: SendState<T>,
}

enum SendState<T> {
    Writing {
        socket: Arc<AsyncFd<Socket2>>,
        buf: T,
        addr: SockAddr,
    },
    Empty,
}

fn send_to(
    socket: &Arc<AsyncFd<Socket2>>,
    buf: &[u8],
    target: &SockAddr,
    cx: &mut Context<'_>,
) -> Poll<Result<usize, io::Error>> {
    loop {
        match socket.poll_write_ready(cx) {
            Poll::Ready(Ok(mut guard)) => {
                match guard.try_io(|fd| fd.get_ref().send_to(buf, target)) {
                    Ok(res) => return Poll::Ready(res),
                    Err(_) => continue,
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }
    }
}

impl<T> Future for Send<T>
where
    T: AsRef<[u8]> + Unpin,
{
    type Output = Result<(), io::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.state {
            SendState::Writing {
                ref socket,
                ref buf,
                ref addr,
            } => {
                let n = match send_to(socket, buf.as_ref(), addr, cx) {
                    Poll::Ready(Ok(n)) => n,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                if n != buf.as_ref().len() {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Other,
                        "failed to send entire packet",
                    )));
                }
            }
            SendState::Empty => panic!("poll a Send after it's done"),
        }

        match ::std::mem::replace(&mut self.state, SendState::Empty) {
            SendState::Writing { .. } => Poll::Ready(Ok(())),
            SendState::Empty => unreachable!(),
        }
    }
}
