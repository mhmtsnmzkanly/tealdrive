#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    WaitingForClientHello,
    Ready,
}
