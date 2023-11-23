use std::collections::{HashMap,HashSet};
use std::net::IpAddr;

pub const PORT:u16 = 9090; 
pub const TIMEOUT_SECS:u8 = 5;
pub const TIMEOUT_NANO:u32 = 0;

pub struct Shared{
    pub filename:String,
    pub rarity_ids:Vec<u32>,
    pub peers_to_chunk:HashMap<u32,HashSet<IpAddr>>,
    pub peers_taken:HashSet<IpAddr>,
    pub peers_latency:HashMap<IpAddr,u16>,
    pub next_index:u32,
	pub peer_count:usize,
}

impl Shared{
	pub fn new(f:String,p_to_c:HashMap<u32,HashSet<IpAddr>>) -> Shared{
		let counter = Shared::peer_count(p_to_c.values().collect::<Vec<&HashSet<IpAddr>>>());
		Shared{
			filename:f,
			rarity_ids:Self::ord_chunks_by_rarity(&p_to_c),
			peers_to_chunk:p_to_c,
			peers_taken:HashSet::new(),
			peers_latency:HashMap::new(),
			next_index:0,
			peer_count:counter,
		}		
	}

	fn ord_chunks_by_rarity(p_to_c:&HashMap<u32,HashSet<IpAddr>>) -> Vec<u32>{
		let mut result:Vec<(u32,u32)> = Vec::new();
		for (id,peers) in p_to_c.iter(){
			result.push((id.clone(),peers.len() as u32));
		}
		result.sort_by(|(_,n1),(_,n2)| n2.cmp(n1));
		result.into_iter().map(|(id,_)|id).collect()
	}

	fn peer_count(p_to_c:Vec<&HashSet<IpAddr>>)->usize{
		let result:HashSet<&IpAddr> = p_to_c.iter().flat_map(|ip_hashset|ip_hashset.iter()).collect();
		result.len()
	}
}
