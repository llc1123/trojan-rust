use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use treebitmap::IpLookupTable;

pub struct ACL {
    blocklist4: IpLookupTable<Ipv4Addr, String>,
    blocklist6: IpLookupTable<Ipv6Addr, String>,
}

impl ACL {
    pub fn new(block_local: bool) -> Self {
        let mut blocklist4: IpLookupTable<Ipv4Addr, String> = IpLookupTable::new();
        let mut blocklist6: IpLookupTable<Ipv6Addr, String> = IpLookupTable::new();
        if block_local {
            blocklist4.insert("127.0.0.0".parse().unwrap(), 8, "loopback".to_string());
            blocklist4.insert("10.0.0.0".parse().unwrap(), 8, "local8".to_string());
            blocklist4.insert("100.64.0.0".parse().unwrap(), 10, "local10".to_string());
            blocklist4.insert("172.16.0.0".parse().unwrap(), 12, "local12".to_string());
            blocklist4.insert("198.18.0.0".parse().unwrap(), 15, "local15".to_string());
            blocklist4.insert("192.168.0.0".parse().unwrap(), 16, "local16".to_string());
            blocklist4.insert("192.0.0.0".parse().unwrap(), 24, "local24".to_string());
            blocklist6.insert("::1".parse().unwrap(), 128, "loopback".to_string());
            blocklist6.insert("fc00::".parse().unwrap(), 7, "unique-local".to_string());
            blocklist6.insert("fe80::".parse().unwrap(), 10, "link-local".to_string());
        }
        ACL {
            blocklist4,
            blocklist6,
        }
    }

    pub fn has_match(&self, address: SocketAddr) -> bool {
        match address {
            SocketAddr::V4(addr) => self.blocklist4.longest_match(*addr.ip()).is_some(),
            SocketAddr::V6(addr) => self.blocklist6.longest_match(*addr.ip()).is_some(),
        }
    }
}
