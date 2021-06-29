use rustls::{Certificate, ServerCertVerifier};

pub(crate) struct SingleCertVerifier {
  certificate: Certificate,
}

impl SingleCertVerifier {
  pub(crate) fn new(certificate: Certificate) -> SingleCertVerifier {
    SingleCertVerifier { certificate }
  }
}

impl ServerCertVerifier for SingleCertVerifier {
  fn verify_server_cert(
    &self,
    _: &rustls::RootCertStore,
    certificates: &[Certificate],
    _: webpki::DNSNameRef,
    _: &[u8],
  ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
    match certificates {
      [end_entity_cert] => {
        if end_entity_cert == &self.certificate {
          Ok(rustls::ServerCertVerified::assertion())
        } else {
          Err(rustls::TLSError::General(
            "unexpected certificate presented".to_owned(),
          ))
        }
      }
      [] => Err(rustls::TLSError::NoCertificatesPresented),
      [..] => Err(rustls::TLSError::General(
        "more than one certificate presented".to_owned(),
      )),
    }
  }
}
