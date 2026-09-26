use std::io;
use std::net::TcpStream;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use super::error::Error;

pub(crate) fn config() -> Result<Arc<ClientConfig>, Error> {
    let loaded = rustls_native_certs::load_native_certs();
    if loaded.certs.is_empty() {
        return Err(Error::Config(
            "the platform certificate store had no usable certificates".into(),
        ));
    }
    let mut roots = RootCertStore::empty();
    let mut added = 0usize;
    for cert in loaded.certs {
        if roots.add(cert).is_ok() {
            added += 1;
        }
    }
    if added == 0 {
        return Err(Error::Config(
            "the platform certificate store could not be loaded".into(),
        ));
    }
    let provider = rustls::crypto::ring::default_provider();
    let config = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|err| Error::Config(err.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

pub(crate) fn wrap(
    tcp: TcpStream,
    host: &str,
    config: &Arc<ClientConfig>,
) -> Result<StreamOwned<ClientConnection, TcpStream>, Error> {
    let name = ServerName::try_from(host.to_string())
        .map_err(|_| Error::Config(format!("not a TLS server name: {host}")))?;
    let conn = ClientConnection::new(Arc::clone(config), name)
        .map_err(|err| Error::Connect(io::Error::other(err.to_string())))?;
    let mut stream = StreamOwned::new(conn, tcp);
    // Finish the handshake before any HTTP bytes are written.
    while stream.conn.is_handshaking() {
        stream
            .conn
            .complete_io(&mut stream.sock)
            .map_err(|err| Error::Connect(io::Error::other(err.to_string())))?;
    }
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::{ServerConfig, ServerConnection, StreamOwned};

    use crate::blocking::Client;

    #[test]
    fn untrusted_https_server_is_refused_before_http() {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert = CertificateDer::from(certified.cert);
        let key =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
        let provider = rustls::crypto::ring::default_provider();
        let server_config = ServerConfig::builder_with_provider(Arc::new(provider))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            let Ok(conn) = ServerConnection::new(Arc::new(server_config)) else {
                return;
            };
            let mut tls = StreamOwned::new(conn, sock);
            let mut buf = [0u8; 32];
            let _ = tls.read(&mut buf);
        });

        let err = Client::builder()
            .base_url(format!("https://127.0.0.1:{port}"))
            .workspace_id("box")
            .connect_timeout(Duration::from_secs(2))
            .read_timeout(Duration::from_secs(2))
            .build()
            .expect("https client")
            .probe()
            .expect_err("self-signed certificate");
        assert!(matches!(err, crate::blocking::Error::Connect(_)), "{err}");
    }
}
