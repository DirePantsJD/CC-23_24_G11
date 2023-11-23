use std::collections::{HashMap,HashSet};
use std::net::{IpAddr, UdpSocket, SocketAddr};
use std::io::{Error, ErrorKind};
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
		chunk_data:&[],
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

fn stop_wait(thread_socket:&UdpSocket,data:&mut Shared) -> (u32,bool){
	let next_chunk_id = data.next_index;
	let mut picked:HashSet<IpAddr> = HashSet::new();
	data.next_index+=1;
	let mut reply:[u8;1500] = [0;1500];
	let mut retries = 0;
	let mut resend = true;
	let mut r:anyhow::Result<()> = Ok(());
	
	loop{
		let peer_ip:IpAddr = if let Some(ip) = peer_picker(&data.peers_to_chunk[&next_chunk_id],&picked,data){
			ip
		} else{
			return (next_chunk_id,false)
		};

		loop{
			if resend{
				r = request_chunk(peer_ip,thread_socket,next_chunk_id,data.filename.clone());
			}
				
			match r {
				Ok(()) => {
					match thread_socket.recv_from(&mut reply){
						Ok((len,source)) =>{
							if ok_handler(len,source,&reply,&mut resend,&thread_socket,next_chunk_id){
								return (next_chunk_id,true);
							}else{
								continue;
							}
						},
						Err(e) => {
							match e.kind(){
								ErrorKind::TimedOut =>{
									resend = true;
									if retries==3{
										retries = 0;
										picked.insert(peer_ip);
										break;
									} else{
										retries+=1;
										continue;
									}
								},
								_ => return (next_chunk_id,false),
							}
					
						},
					}
				},
				//IDK WHAT TO DO HERE YET MAYBE DIS
				Err(_) => {resend = true},
			}
		}		
	}
}

fn ok_handler(len:usize,source:SocketAddr,buffer:&[u8],resend:&mut bool,thread_socket:&UdpSocket,next_chunk_id:u32) -> bool{
	if let Ok(mut packet) = fsnp::Protocol::read_packet(&buffer,len as u16){
		packet.action = 0;
		let (ack_reply,len) = packet.build_packet().unwrap();

		if let Err(_) = thread_socket.send_to(&ack_reply[0..len as usize],source){
			*resend = true;
			return false;
		}

		if packet.chunk_id!=next_chunk_id{
			*resend = false;
			return false;
		}else{
			write_chunk(packet);
			return true;
		}
	} else{
		false
	}
}

// fn err_handler(e:anyhow::Result<()>,resend:&bool,retries:&i32,picked:&HashSet<IpAddr>,peer_ip:IpAddr) -> bool{
// 	match e.kind(){
// 		ErrorKind::TimedOut =>{
// 			*resend = true;
// 			if retries==3{
// 				retries = 0;
// 				picked.insert(peer_ip);
// 				break;
// 			}else{
// 				retries+=1;
// 				continue;
// 			}
// 		},
// 		_ => return (next_chunk_id,false),
// 	}
// }




// PLACEHOLDER
fn write_chunk(packet:fsnp::Protocol){
	
}













//
