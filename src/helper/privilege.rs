#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperPrivilegeMode {
    DevelopmentNoPrivilegeDrop,
    SetuidRoot,
    FileCapabilities,
    SystemdMediated,
}
