use std::future::Future;
use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};

use axum::Router;
#[cfg(target_os = "linux")]
use socket2::{Domain, SockRef, Type};
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use crate::config::Listen;
use crate::error::{AppError, AppResult};
#[cfg(target_os = "linux")]
use crate::systemd;

pub enum Listener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix {
        listener: UnixListener,
        created: Option<PathBuf>,
    },
}

impl Listener {
    pub async fn acquire(listen: &Listen) -> AppResult<Self> {
        match listen {
            Listen::Tcp(addr) => Ok(Self::Tcp(TcpListener::bind(*addr).await?)),
            #[cfg(unix)]
            Listen::Unix(path) => {
                remove_stale_socket(path).await?;
                Ok(Self::Unix {
                    listener: UnixListener::bind(path)?,
                    created: Some(path.clone()),
                })
            }
            #[cfg(target_os = "linux")]
            Listen::Systemd => Self::inherited(),
        }
    }

    #[cfg(target_os = "linux")]
    fn inherited() -> AppResult<Self> {
        match <[OwnedFd; 1]>::try_from(systemd::take_listen_fds()?) {
            Ok([fd]) => Self::from_fd(fd),
            Err(fds) => Err(AppError::Config(format!(
                "PINEPODS_LISTEN is systemd, but systemd passed {} sockets; PinePods listens on exactly one",
                fds.len()
            ))),
        }
    }

    #[cfg(target_os = "linux")]
    fn from_fd(fd: OwnedFd) -> AppResult<Self> {
        let socket = SockRef::from(&fd);
        match (socket.r#type()?, socket.is_listener()?, socket.local_addr()?.domain()) {
            (Type::STREAM, true, Domain::IPV4 | Domain::IPV6) => {
                let listener = std::net::TcpListener::from(fd);
                listener.set_nonblocking(true)?;
                Ok(Self::Tcp(TcpListener::from_std(listener)?))
            }
            (Type::STREAM, true, Domain::UNIX) => {
                let listener = std::os::unix::net::UnixListener::from(fd);
                listener.set_nonblocking(true)?;
                Ok(Self::Unix {
                    listener: UnixListener::from_std(listener)?,
                    created: None,
                })
            }
            _ => Err(AppError::Config(
                "The socket systemd passed is not a listening TCP or unix stream socket; use ListenStream= with Accept=no".to_string(),
            )),
        }
    }

    pub fn endpoint(&self) -> io::Result<String> {
        match self {
            Self::Tcp(listener) => Ok(format!("http://{}", listener.local_addr()?)),
            #[cfg(unix)]
            Self::Unix { listener, .. } => {
                let addr = listener.local_addr()?;
                Ok(match addr.as_pathname() {
                    Some(path) => format!("unix:{}", path.display()),
                    None => format!("unix:{addr:?}"),
                })
            }
        }
    }

    pub async fn serve(self, app: Router, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
        match self {
            Self::Tcp(listener) => axum::serve(listener, app).with_graceful_shutdown(shutdown).await,
            #[cfg(unix)]
            Self::Unix { listener, created } => {
                let served = axum::serve(listener, app).with_graceful_shutdown(shutdown).await;
                let removed = match created {
                    Some(path) => tokio::fs::remove_file(path).await,
                    None => Ok(()),
                };
                served.and(removed)
            }
        }
    }
}

#[cfg(unix)]
async fn remove_stale_socket(path: &Path) -> AppResult<()> {
    match tokio::fs::symlink_metadata(path).await {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(metadata) if !metadata.file_type().is_socket() => Err(AppError::Config(format!(
            "PINEPODS_LISTEN {} exists and is not a socket",
            path.display()
        ))),
        Ok(_) => match UnixStream::connect(path).await {
            Ok(_) => Err(AppError::Config(format!(
                "PINEPODS_LISTEN {} is in use by another process",
                path.display()
            ))),
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => Ok(tokio::fs::remove_file(path).await?),
            Err(e) => Err(e.into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use socket2::{SockAddr, Socket};
    use std::net::Ipv4Addr;

    #[cfg(unix)]
    struct SocketPath(PathBuf);

    #[cfg(unix)]
    impl SocketPath {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("pinepods-{}.sock", uuid::Uuid::new_v4())))
        }
    }

    #[cfg(unix)]
    impl Drop for SocketPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn binds_the_configured_address() {
        let configured = Listen::Tcp("127.0.0.1:0".parse().unwrap());
        assert!(matches!(
            Listener::acquire(&configured).await,
            Ok(Listener::Tcp(listener)) if listener.local_addr().unwrap().ip() == Ipv4Addr::LOCALHOST,
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn replaces_a_stale_socket() {
        let path = SocketPath::new();
        drop(std::os::unix::net::UnixListener::bind(&path.0).unwrap());
        let listener = Listener::acquire(&Listen::Unix(path.0.clone())).await;
        assert!(matches!(listener, Ok(Listener::Unix { created: Some(_), .. })));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refuses_a_socket_in_use() {
        let path = SocketPath::new();
        let _live = std::os::unix::net::UnixListener::bind(&path.0).unwrap();
        assert!(matches!(
            Listener::acquire(&Listen::Unix(path.0.clone())).await,
            Err(AppError::Config(_)),
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refuses_a_path_that_is_not_a_socket() {
        let path = SocketPath::new();
        std::fs::write(&path.0, b"").unwrap();
        assert!(matches!(
            Listener::acquire(&Listen::Unix(path.0.clone())).await,
            Err(AppError::Config(_)),
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn removes_its_socket_on_shutdown() {
        let path = SocketPath::new();
        let listener = Listener::acquire(&Listen::Unix(path.0.clone())).await.unwrap();
        listener.serve(Router::new(), async {}).await.unwrap();
        assert!(!path.0.exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn inherits_a_tcp_listener() {
        let inherited = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = inherited.local_addr().unwrap();
        assert!(matches!(
            Listener::from_fd(OwnedFd::from(inherited)),
            Ok(Listener::Tcp(listener)) if listener.local_addr().unwrap() == addr,
        ));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn inherits_a_unix_listener() {
        let path = SocketPath::new();
        let fd = OwnedFd::from(std::os::unix::net::UnixListener::bind(&path.0).unwrap());
        assert!(matches!(
            Listener::from_fd(fd),
            Ok(Listener::Unix { listener, created: None })
                if listener.local_addr().unwrap().as_pathname() == Some(path.0.as_path()),
        ));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn leaves_an_inherited_socket_in_place() {
        let path = SocketPath::new();
        let fd = OwnedFd::from(std::os::unix::net::UnixListener::bind(&path.0).unwrap());
        let listener = Listener::from_fd(fd).unwrap();
        listener.serve(Router::new(), async {}).await.unwrap();
        assert!(path.0.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refuses_a_sequential_packet_socket() {
        let path = SocketPath::new();
        let socket = Socket::new(Domain::UNIX, Type::SEQPACKET, None).unwrap();
        socket.bind(&SockAddr::unix(&path.0).unwrap()).unwrap();
        socket.listen(1).unwrap();
        assert!(matches!(Listener::from_fd(OwnedFd::from(socket)), Err(AppError::Config(_))));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refuses_a_socket_that_is_not_listening() {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        socket.bind(&std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into()).unwrap();
        assert!(matches!(Listener::from_fd(OwnedFd::from(socket)), Err(AppError::Config(_))));
    }
}
