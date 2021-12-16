mod config;

use std::{
    convert::TryFrom,
    io::{self, Write},
    net::TcpStream,
    sync::Arc,
};

use log::error;
use rustls::{RootCertStore, ServerName};

use self::config::Config;
use ironrdp_client::{process_active_stage, process_connection_sequence, RdpError, UpgradedStream};

mod danger {

    pub struct NoCertificateVerification {}

    impl rustls::client::ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::Certificate,
            _intermediates: &[rustls::Certificate],
            _server_name: &rustls::ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: std::time::SystemTime,
        ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::ServerCertVerified::assertion())
        }
    }
}

fn main() {
    let config = Config::parse_args();
    setup_logging(config.log_file.as_str()).expect("failed to initialize logging");

    let exit_code = match run(config) {
        Ok(_) => {
            println!("RDP successfully finished");

            exitcode::OK
        }
        Err(RdpError::IOError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
            error!("{}", e);
            println!("The server has terminated the RDP session");

            exitcode::NOHOST
        }
        Err(ref e) => {
            error!("{}", e);
            println!("RDP failed because of {}", e);

            match e {
                RdpError::IOError(_) => exitcode::IOERR,
                RdpError::ConnectionError(_) => exitcode::NOHOST,
                _ => exitcode::PROTOCOL,
            }
        }
    };

    std::process::exit(exit_code);
}

fn setup_logging(log_file: &str) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S:%6f]"),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file(log_file)?)
        .apply()?;

    Ok(())
}

fn run(config: Config) -> Result<(), RdpError> {
    let addr = socket_addr_to_string(config.routing_addr);
    let mut stream = TcpStream::connect(addr.as_str()).map_err(RdpError::ConnectionError)?;

    let (connection_sequence_result, mut stream) = process_connection_sequence(
        &mut stream,
        &config.routing_addr,
        &config.input,
        establish_tls,
    )?;

    process_active_stage(&mut stream, config.input, connection_sequence_result)?;

    Ok(())
}

fn establish_tls(
    stream: impl io::Read + io::Write,
) -> Result<UpgradedStream<impl io::Read + io::Write>, RdpError> {
    let root_store = RootCertStore::empty();

    let mut client_config = rustls::ClientConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_safe_default_protocol_versions()?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    client_config
        .dangerous()
        .set_certificate_verifier(Arc::new(danger::NoCertificateVerification {}));

    let config_ref = Arc::new(client_config);
    let dns_name = ServerName::try_from("stub-name.com").expect("invalid DNS name");
    let tls_session = rustls::ClientConnection::new(config_ref, dns_name)?;
    let mut tls_stream = rustls::StreamOwned::new(tls_session, stream);
    // handshake
    tls_stream.flush()?;

    let cert = tls_stream
        .conn
        .peer_certificates()
        .ok_or(RdpError::TlsConnectorError(
            rustls::Error::NoCertificatesPresented,
        ))?;
    let server_public_key = get_tls_peer_pubkey(cert[0].as_ref().to_vec())?;

    Ok(UpgradedStream {
        stream: tls_stream,
        server_public_key,
    })
}

pub fn socket_addr_to_string(socket_addr: std::net::SocketAddr) -> String {
    format!("{}:{}", socket_addr.ip(), socket_addr.port())
}

pub fn get_tls_peer_pubkey(cert: Vec<u8>) -> io::Result<Vec<u8>> {
    let res = x509_parser::parse_x509_der(&cert[..])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid der certificate."))?;
    let public_key = res.1.tbs_certificate.subject_pki.subject_public_key;

    Ok(public_key.data.to_vec())
}
