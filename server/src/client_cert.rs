use tonic::transport::Certificate;
use x509_parser::prelude::*;

/// A CommonName extracted from a mTLS client certificate
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientName(String);

impl ClientName {
    /// Extract subject CommonName fields from a DER-encoded certificate.
    /// Pre-condition: Certificate validated by the tonic server.
    /// Pre-condition: Root CA will not sign nonconforming certificates.
    /// Might panic if pre-conditions are not met.
    pub fn from_cert(cert: &Certificate) -> Self {
        let (_, x509) = parse_x509_certificate(cert.get_ref()).expect("Could not decode DER data");
        let s = x509.subject();
        let name = s
            .iter_common_name()
            .next()
            .expect("Client CN missing from certificate");
        let cn = name.as_str().expect("Could not get client CN as string");
        Self(cn.to_owned())
    }

    /// Reads certificate info from a Tonic request
    pub fn from_request<T>(request: &tonic::Request<T>) -> Option<Self> {
        use std::borrow::Borrow;

        let arc = request.peer_certs()?;
        let certs: &Vec<Certificate> = arc.borrow();
        Some(ClientName::from_cert(certs.first()?))
    }
}
