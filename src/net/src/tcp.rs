use crate::*;

pub struct TcpStream {
    inner: mio::net::TcpStream,
}

impl TcpStream {
    pub fn connect(addr: SocketAddr) -> Result<Self> {
        let inner = mio::net::TcpStream::connect(addr)?;

        Ok(Self { inner })
    }

    pub fn is_established(&self) -> bool {
        self.peer_addr().is_ok()
    }

    pub fn from_std(stream: std::net::TcpStream) -> Self {
        let inner = mio::net::TcpStream::from_std(stream);

        Self { inner }
    }
}

impl Deref for TcpStream {
    type Target = mio::net::TcpStream;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl event::Source for TcpStream {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        self.inner.register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        self.inner.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        self.inner.deregister(registry)
    }
}

pub struct TcpListener {
    inner: mio::net::TcpListener,
}

impl Deref for TcpListener {
    type Target = mio::net::TcpListener;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<TcpListener> {
        // we create from a std TcpListener so SO_REUSEADDR is not set for us
        let l = std::net::TcpListener::bind(addr)?;
        l.set_nonblocking(true)?;

        let inner = mio::net::TcpListener::from_std(l);

        Ok(Self { inner })
    }

    pub fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        self.inner
            .accept()
            .map(|(stream, addr)| (TcpStream { inner: stream }, addr))
    }
}

impl event::Source for TcpListener {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        self.inner.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        self.inner.deregister(registry)
    }
}
