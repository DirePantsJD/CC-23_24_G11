struct PeersWithFile {
    name: String,
    peers: Vec<IpAddr>,
}

impl PeersWithFile {
    pub fn new() -> Self {
        PeersWithFile {
            name: String::new(),
            peers: Vec::new(),
        }
    }
}
