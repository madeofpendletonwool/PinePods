use std::future::Future;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};

use axum::Router;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use crate::config::Listen;
use crate::error::{AppError, AppResult};

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
}
