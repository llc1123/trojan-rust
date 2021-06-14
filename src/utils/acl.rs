use std::net::SocketAddr;

pub struct ACL {
    block_local: bool,
}

impl ACL {
    pub fn new(block_local: bool) -> Self {
        ACL { block_local }
    }

    pub fn has_match(&self, address: &SocketAddr) -> bool {
        if !self.block_local {
            return false;
        }
        match address {
            SocketAddr::V4(addr) => addr.ip().is_global(),
            SocketAddr::V6(addr) => addr.ip().is_global(),
        }
    }
}
