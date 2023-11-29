use std::collections::HashSet;
use std::net::{IpAddr, UdpSocket, SocketAddr};
use std::io::ErrorKind;
use anyhow;

mod shared;
use shared::Shared;
use fsnp;

fn peer_picker(peers:&HashSet<IpAddr>,avoid_peers:&HashSet<IpAddr>,data:&Shared) -> Option<IpAddr>{
	if peers.is_empty(){
		return None;
	}

	let valid_peers:HashSet<&IpAddr> = peers.iter().filter(|ip|{
		!avoid_peers.contains(ip)
	}).collect();

	if valid_peers.is_empty(){
		return None;
	}
	
	let never_taken_peers:HashSet<&IpAddr> = valid_peers.iter().filter(|ip|{
		!data.peers_latency.contains_key(ip)
	}).cloned().collect();
	
	let mut ordered_peers:Vec<&IpAddr> =
		if never_taken_peers.is_empty(){
			Vec::from_iter(valid_peers.into_iter())
		} else{
			Vec::from_iter(never_taken_peers.into_iter())
		};
	
	ordered_peers.sort_by(|ip1,ip2|{
		data.peers_latency.get(ip1).cmp(&data.peers_latency.get(ip2))
	});

	Some(ordered_peers[0].clone())
}

fn request_chunk(peer_ip:IpAddr,local_socket:&UdpSocket,chunkID:u32,filename:String) -> anyhow::Result<()>{
	let p:Option<([u8;fsnp::MAX_PACKET_SIZE],u16)> = fsnp::Protocol{
		action:1,
		chunk_id:chunkID,
		filename:&filename,
		len_chunk:0,
		chunk_data:[0;fsnp::MAX_CHUNK_SIZE],
	}.build_packet();

	match p{
		Some((packet,len))=>{
			match local_socket.send_to(&packet[0..len as usize],(peer_ip,shared::PORT)){
				Ok(_) => anyhow::Ok(()),
				Err(e) => anyhow::bail!(e.to_string()),
			}
		},
		None => anyhow::bail!("ERROR BUILDING PACKET,FILENAME>25B OR CHUNK>1420B"),
	}
}

fn stop_wait(thread_socket:&UdpSocket,data:&mut Shared) -> anyhow::Result<(u32,bool)>{
	let next_chunk_id = data.next_index;
	data.next_index+=1;

	let mut picked:HashSet<IpAddr> = HashSet::new();
	let mut reply:[u8;1500] = [0;1500];
	let mut request:anyhow::Result<()> = Ok(());
	
	let mut retries = 0;
	let mut resend = true;
	let mut fetch_peer:bool = true;
	let mut peer_ip:IpAddr = thread_socket.local_addr()?.ip();

	loop{
		if fetch_peer{
			if let Some(ip) = peer_picker(&data.peers_to_chunk[&next_chunk_id],&picked,data){
				peer_ip = ip;
			}
			else{
				return Ok((next_chunk_id,false))
			}
			fetch_peer = false;
		}

		if resend{
			request = request_chunk(peer_ip,thread_socket,next_chunk_id,data.filename.clone());
		}

		if let Ok(()) = request {
			match thread_socket.recv_from(&mut reply){
				Ok((len,source))=>{
					if let Ok(mut packet) = fsnp::Protocol::read_packet(&reply,len as u16){
						if let Ok(()) = send_ack(&thread_socket,packet.clone(),source){
							if packet.chunk_id == next_chunk_id{
								write_chunk(packet);
								return Ok((next_chunk_id,true));
							}
							resend = false;
						}
					}
					// bad packet; failed parsing
					else{
						resend = true;						
					}
				},
				// socket receive timeout => retry mechanism
				Err(e)=>{
					match e.kind(){
						ErrorKind::TimedOut=>{
							if retries==3{
								retries = 0;
								fetch_peer = true;
								picked.insert(peer_ip);
							}
							else{
								retries+=1;
							}
							resend = true;
						},
						//Handle other errors
						_=>{
							return Ok((next_chunk_id,false));
						}
					}
				},
			}	
		}
		// failed send request
		else{
			resend = true;
		}
	}
}

// PLACEHOLDER
fn write_chunk(packet:fsnp::Protocol){
	
}

fn send_ack(local_socket:&UdpSocket,mut peer_response:fsnp::Protocol,peer:SocketAddr) -> anyhow::Result<()>{
	peer_response.action = 0;
	peer_response.len_chunk = 0;
	peer_response.chunk_data=[0;fsnp::MAX_CHUNK_SIZE];
	
	if let Some((ack,len)) = peer_response.build_packet(){
		if let Ok(_) = local_socket.send_to(&ack[0..len as usize],peer){
			()
		}
	}
	anyhow::bail!("Error sending or building ack")
}













//
