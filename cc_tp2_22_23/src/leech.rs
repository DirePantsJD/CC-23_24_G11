use anyhow;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::fsnp::*;
use crate::partial_file::*;
use crate::shared::*;

const MAX_LEECH_THREADS: u8 = 5;

fn peer_picker(
    chunk_id: u32,
    avoid_peers: &HashSet<IpAddr>,
    data_rwl: &Arc<RwLock<Shared>>,
) -> Option<IpAddr> {
    let mut peers: HashSet<IpAddr> = HashSet::new();

    if let Ok(data) = data_rwl.read() {
        peers.extend(data.peers_to_chunk[&chunk_id].iter());
    } else {
        return None;
    }

    if peers.is_empty() {
        return None;
    }

    let valid_peers: HashSet<&IpAddr> = peers
        .iter()
        .filter(|ip| !avoid_peers.contains(ip))
        .collect();

    if valid_peers.is_empty() {
        return None;
    }

    if let Ok(mut data) = data_rwl.write() {
        let never_taken_peers: HashSet<&IpAddr> = valid_peers
            .iter()
            .filter(|ip| !data.peers_latency.contains_key(ip))
            .cloned()
            .collect();

        let mut ordered_peers: Vec<&IpAddr> = if never_taken_peers.is_empty() {
            Vec::from_iter(valid_peers.into_iter())
        } else {
            Vec::from_iter(never_taken_peers.into_iter())
        };

        ordered_peers.sort_by(|ip1, ip2| {
            data.peers_latency
                .get(ip1)
                .cmp(&data.peers_latency.get(ip2))
        });

        data.peers_taken.insert(ordered_peers[0].clone());

        Some(ordered_peers[0].clone())
    } else {
        None
    }
}

fn request_chunk(
    peer_ip: IpAddr,
    local_socket: &UdpSocket,
    chunk_id: u32,
    filename: String,
) -> anyhow::Result<()> {
    let p: Option<([u8; MAX_PACKET_SIZE], u16)> = Protocol {
        action: 1,
        chunk_id,
        filename: &filename,
        len_chunk: 0,
        chunk_data: [0; MAX_CHUNK_SIZE],
    }
    .build_packet();

    match p {
        Some((packet, len)) => {
            match local_socket
                .send_to(&packet[0..len as usize], (peer_ip, PORT))
            {
                Ok(_) => anyhow::Ok(()),
                Err(e) => anyhow::bail!(e.to_string()),
            }
        }
        None => {
            anyhow::bail!("ERROR BUILDING PACKET,FILENAME>25B OR CHUNK>1420B")
        }
    }
}

fn stop_wait(
    thread_socket: &UdpSocket,
    data_rwl: &Arc<RwLock<Shared>>,
    filename: &String,
    file: &mut File,
) -> anyhow::Result<(u32, bool)> {
    let mut next_chunk_id: u32 = 0;
    let mut max_chunk_id: u32 = 0;

    if let Ok(mut data) = data_rwl.write() {
        next_chunk_id = data.next_index;
        data.next_index += 1;
        max_chunk_id = data.peers_to_chunk.len() as u32;
    } else {
        anyhow::bail!("stop_wait: Failed Write Lock");
    }

    if next_chunk_id >= max_chunk_id {
        return Ok((max_chunk_id, true));
    }

    let mut picked: HashSet<IpAddr> = HashSet::new();
    let mut reply: [u8; 1500] = [0; 1500];
    let mut request: anyhow::Result<()> = Ok(());

    let mut retries = 0;
    let mut resend = true;
    let mut fetch_peer: bool = true;
    let mut peer_ip: IpAddr = thread_socket.local_addr()?.ip();

    loop {
        if fetch_peer {
            if let Some(ip) = peer_picker(next_chunk_id, &picked, &data_rwl) {
                peer_ip = ip;
            } else {
                return Ok((next_chunk_id, false));
            }
            fetch_peer = false;
        }

        if resend {
            request = request_chunk(
                peer_ip,
                thread_socket,
                next_chunk_id,
                filename.clone(),
            );
        }

        if let Ok(()) = request {
            match thread_socket.recv_from(&mut reply) {
                Ok((len, source)) => {
                    if let Ok(packet) =
                        Protocol::read_packet(&reply, len as u16)
                    {
                        if let Ok(()) =
                            send_ack(&thread_socket, packet.clone(), source)
                        {
                            if packet.chunk_id == next_chunk_id {
                                if let Ok(_) = write_block(
                                    file,
                                    max_chunk_id - 1,
                                    packet.len_chunk as u32,
                                    packet.chunk_id,
                                    &packet.chunk_data,
                                ) {
                                    return Ok((next_chunk_id, true));
                                } else {
                                    resend = true;
                                    eprintln!("Failed to receive block ");
                                }
                            } else {
                                resend = false;
                            }
                        }
                    }
                    // bad packet; failed parsing
                    else {
                        resend = true;
                    }
                }
                // socket receive timeout => retry mechanism
                Err(e) => {
                    match e.kind() {
                        ErrorKind::TimedOut => {
                            if retries == 3 {
                                retries = 0;
                                fetch_peer = true;
                                picked.insert(peer_ip);
                            } else {
                                retries += 1;
                            }
                            resend = true;
                        }
                        //Handle other errors
                        _ => {
                            return Ok((next_chunk_id, false));
                        }
                    }
                }
            }
        }
        // failed send request
        else {
            resend = true;
        }
    }
}

fn send_ack(
    local_socket: &UdpSocket,
    mut peer_response: Protocol,
    peer: SocketAddr,
) -> anyhow::Result<()> {
    peer_response.action = 0;
    peer_response.len_chunk = 0;
    peer_response.chunk_data = [0; MAX_CHUNK_SIZE];

    if let Some((ack, len)) = peer_response.build_packet() {
        if let Ok(_) = local_socket.send_to(&ack[0..len as usize], peer) {
            ()
        }
    }
    anyhow::bail!("Error sending or building ack")
}

pub fn download_file(
    file_size: u32,
    filename: String,
    p_to_c: HashMap<u32, HashSet<IpAddr>>,
    // local_ip: String,
) {
    let data_unsafe: Shared = Shared::new(filename.clone(), p_to_c);
    let nthreads: usize =
        std::cmp::max(MAX_LEECH_THREADS as usize, data_unsafe.peer_count);
    let max_chunks_id: u32 = data_unsafe.peers_to_chunk.len() as u32;

    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    let data: Arc<RwLock<Shared>> = Arc::new(RwLock::new(data_unsafe));
    let chunks_received: Arc<RwLock<HashSet<u32>>> =
        Arc::new(RwLock::new(HashSet::new()));
    let failed_chunks: Arc<RwLock<HashSet<u32>>> =
        Arc::new(RwLock::new(HashSet::new()));

    for _ in 0..nthreads {
        let t_handler = spawn(
            file_size,
            filename.clone(),
            Arc::clone(&data),
            Arc::clone(&chunks_received),
            Arc::clone(&failed_chunks),
            max_chunks_id.clone(),
        );

        handles.push(t_handler);
    }

    for t in handles {
        t.join().unwrap();
    }
}

fn spawn(
    file_size: u32,
    filename: String,
    // ip: String,
    data: Arc<RwLock<Shared>>,
    chunks_received: Arc<RwLock<HashSet<u32>>>,
    chunks_failed: Arc<RwLock<HashSet<u32>>>,
    max_id: u32,
) -> thread::JoinHandle<()> {
    let t_handler = thread::spawn(move || {
        // if let Ok(socket) = UdpSocket::bind(ip + ":0") {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            let n_blocks = if file_size % MAX_CHUNK_SIZE as u32 == 0 {
                file_size / MAX_CHUNK_SIZE as u32
            } else {
                file_size / MAX_CHUNK_SIZE as u32 + 1
            };
            let mut file =
                create_part_file(&filename, file_size, n_blocks).unwrap();
            loop {
                match stop_wait(&socket, &data, &filename, &mut file) {
                    Ok((id, success)) => {
                        if id == max_id {
                            break;
                        } else {
                            let target = if success {
                                &chunks_received
                            } else {
                                &chunks_failed
                            };

                            if let Ok(mut aquired) = target.write() {
                                aquired.insert(id);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    });

    t_handler
}
