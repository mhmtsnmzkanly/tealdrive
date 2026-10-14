//! Standard Noise NK, shared by native backend and browser WebAssembly.
//! NK authenticates the responder only if its static key is independently trusted.
pub type Transport = snow::TransportState;
pub const PATTERN: &str = "Noise_NK_25519_ChaChaPoly_SHA256";
pub const PROLOGUE: &[u8] = b"Tealdrive WebSocket encrypted records v1";
pub const MAX_RECORD: usize = 65535;
pub const MAX_FRAGMENT: usize = 60000;
pub fn builder() -> snow::Builder<'static> {
    snow::Builder::new(PATTERN.parse().expect("fixed Noise pattern"))
        .prologue(PROLOGUE)
        .expect("fixed prologue")
}
pub fn responder(key: &[u8]) -> Result<snow::HandshakeState, snow::Error> {
    if key.len() != 32 {
        return Err(snow::Error::Input);
    }
    builder().local_private_key(key)?.build_responder()
}
pub fn public_key(key: &[u8]) -> Result<Vec<u8>, snow::Error> {
    if key.len() != 32 {
        return Err(snow::Error::Input);
    }
    // An initiator can derive a public key through the resolver's DH primitive.
    // Use the same specified Curve25519 implementation as Noise.
    let mut dh = snow::resolvers::CryptoResolver::resolve_dh(
        &snow::resolvers::DefaultResolver,
        &snow::params::DHChoice::Curve25519,
    )
    .ok_or(snow::Error::Input)?;
    dh.set(key);
    let public = dh.pubkey().to_vec();
    Ok(public)
}
pub struct Client {
    handshake: Option<snow::HandshakeState>,
    transport: Option<snow::TransportState>,
}
impl Client {
    pub fn new(key: &[u8]) -> Result<Self, snow::Error> {
        if key.len() != 32 {
            return Err(snow::Error::Input);
        }
        Ok(Self {
            handshake: Some(builder().remote_public_key(key)?.build_initiator()?),
            transport: None,
        })
    }
    pub fn start(&mut self) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0; MAX_RECORD];
        let n = self
            .handshake
            .as_mut()
            .ok_or(snow::Error::Input)?
            .write_message(&[], &mut out)?;
        out.truncate(n);
        Ok(out)
    }
    pub fn finish(&mut self, message: &[u8]) -> Result<(), snow::Error> {
        let mut out = vec![0; MAX_RECORD];
        let mut state = self.handshake.take().ok_or(snow::Error::Input)?;
        if state.read_message(message, &mut out)? != 0 {
            return Err(snow::Error::Input);
        }
        self.transport = Some(state.into_transport_mode()?);
        Ok(())
    }
    pub fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0; MAX_RECORD];
        let n = self
            .transport
            .as_mut()
            .ok_or(snow::Error::Input)?
            .write_message(plain, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
    pub fn open(&mut self, cipher: &[u8]) -> Result<Vec<u8>, snow::Error> {
        let mut out = vec![0; MAX_RECORD];
        let n = self
            .transport
            .as_mut()
            .ok_or(snow::Error::Input)?
            .read_message(cipher, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
}
#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct NoiseClient(super::Client);
    fn error(_: snow::Error) -> JsValue {
        JsValue::from_str("ENCRYPTED_CHANNEL_FAILED")
    }
    #[wasm_bindgen]
    impl NoiseClient {
        #[wasm_bindgen(constructor)]
        pub fn new(key: &[u8]) -> Result<NoiseClient, JsValue> {
            super::Client::new(key).map(Self).map_err(error)
        }
        pub fn start(&mut self) -> Result<Vec<u8>, JsValue> {
            self.0.start().map_err(error)
        }
        pub fn finish(&mut self, message: &[u8]) -> Result<(), JsValue> {
            self.0.finish(message).map_err(error)
        }
        pub fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.0.seal(plain).map_err(error)
        }
        pub fn open(&mut self, cipher: &[u8]) -> Result<Vec<u8>, JsValue> {
            self.0.open(cipher).map_err(error)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nk_identity_replay_tamper_and_reconnect() {
        let key = builder().generate_keypair().unwrap();
        let mut server = responder(&key.private).unwrap();
        let mut client = Client::new(&key.public).unwrap();
        let first = client.start().unwrap();
        let mut plain = [0; MAX_RECORD];
        assert_eq!(server.read_message(&first, &mut plain).unwrap(), 0);
        let mut response = [0; MAX_RECORD];
        let n = server.write_message(&[], &mut response).unwrap();
        client.finish(&response[..n]).unwrap();
        let mut server = server.into_transport_mode().unwrap();
        assert_eq!(public_key(&key.private).unwrap(), key.public);
        let cipher = client.seal(b"secret").unwrap();
        assert_eq!(server.read_message(&cipher, &mut plain).unwrap(), 6);
        assert_eq!(&plain[..6], b"secret");
        assert!(server.read_message(&cipher, &mut plain).is_err());
        let n = server.write_message(b"reply", &mut response).unwrap();
        assert_eq!(client.open(&response[..n]).unwrap(), b"reply");
        let mut cipher = client.seal(b"tamper").unwrap();
        cipher[0] ^= 1;
        assert!(server.read_message(&cipher, &mut plain).is_err());
        let wrong = builder().generate_keypair().unwrap();
        let mut wrong_client = Client::new(&wrong.public).unwrap();
        let mut fresh_server = responder(&key.private).unwrap();
        assert!(fresh_server
            .read_message(&wrong_client.start().unwrap(), &mut plain)
            .is_err());
        // Each connection receives fresh ephemeral keys and directional nonces.
        let mut fresh_client = Client::new(&key.public).unwrap();
        assert_ne!(fresh_client.start().unwrap(), first);
        assert!(Client::new(&[0; 31]).is_err());
    }
}
