use anyhow;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::net::{IpAddr, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::fsnp::*;
use crate::fstp::*;
use crate::partial_file::*;
use crate::shared::*;

const MAX_LEECH_THREADS: u8 = 5;
const DEFAULT_TIMEOUT_MS: u32 = 500;
const TIMEOUT_MULTIPLIER: f64 = 1.5;

fn peer_picker(
    chunk_id: u32,
    avoid_peers: &HashSet<IpAddr>,
    data_rwl: &Arc<RwLock<Shared>>,
) -> Option<(IpAddr, f64)> //(ip,milisec)
{
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

        if data.peers_latency.contains_key(ordered_peers[0]) {
            Some((
                ordered_peers[0].clone(),
                data.peers_latency.get(ordered_peers[0]).unwrap().clone()
                    as f64,
            ))
        } else {
            Some((ordered_peers[0].clone(), 0.0))
        }
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
                Ok(_) => {
                    println!(
                        "\n\nvvv FSNP OUT vvv\n{:?}",
                        Protocol::read_packet(&packet, len)?.to_string()
                    );
                    anyhow::Ok(())
                }
                Err(e) => anyhow::bail!(e.to_string()),
            }
        }
        None => {
            anyhow::bail!("ERROR BUILDING PACKET,FILENAME>25B OR CHUNK>1420B")
        }
    }
}

fn stop_wait(
    tracker: &Arc<Mutex<TcpStream>>,
    thread_socket: &UdpSocket,
    data_rwl: &Arc<RwLock<Shared>>,
    file: &mut Arc<File>,
) -> anyhow::Result<(u32, bool)> {
    let mut next_chunk_id: u32 = 0;
    let mut max_chunk_id: u32 = 0;
    let mut filename: String = "".to_string();

    if let Ok(mut data) = data_rwl.write() {
        next_chunk_id = data.next_index;
        data.next_index += 1;
        max_chunk_id = data.peers_to_chunk.len() as u32;
        filename = data.filename.clone();
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
    let mut latency_ms: f64 = 0.0;
    let mut current_rtt = Instant::now();

    loop {
        if fetch_peer {
            if let Some((ip, lat)) =
                peer_picker(next_chunk_id, &picked, &data_rwl)
            {
                peer_ip = ip;
                latency_ms = lat;
                let timeout: Duration = if lat > 0.0 {
                    Duration::new(
                        0,
                        (lat * TIMEOUT_MULTIPLIER) as u32 * 10_u32.pow(6),
                    )
                } else {
                    Duration::new(0, DEFAULT_TIMEOUT_MS * 10_u32.pow(6))
                };
                thread_socket.set_read_timeout(Some(timeout))?;
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
            current_rtt = Instant::now();
        }
        dbg!(&peer_ip, &next_chunk_id, thread::current().id());

        if let Ok(()) = request {
            match thread_socket.recv_from(&mut reply) {
                Ok((len, _)) => {
                    if let Ok(packet) =
                        Protocol::read_packet(&reply, len as u16)
                    {
                        println!(
                            "\n\nvvv FSNP IN vvv \n{:?}",
                            packet.to_string()
                        );
                        if packet.chunk_id == next_chunk_id {
                            println!("chunkkk:{}", packet.chunk_id);
                            let duration = current_rtt.elapsed().as_millis();
                            if let Ok(_) = write_block(
                                file,
                                max_chunk_id - 1,
                                packet.len_chunk as u32,
                                packet.chunk_id,
                                &packet.chunk_data,
                            ) {
                                // update peer latency if necessary
                                if duration as f64
                                    >= latency_ms * TIMEOUT_MULTIPLIER
                                    || duration as f64
                                        <= latency_ms
                                            * (2_f64 - TIMEOUT_MULTIPLIER)
                                {
                                    update_peer_latency(
                                        duration as u16,
                                        &peer_ip,
                                        data_rwl,
                                    );
                                }
                                assert_eq!(&filename, &packet.filename);
                                update_tracker_chunks(
                                    &packet, &filename, &tracker,
                                )?;
                                return Ok((next_chunk_id, true));
                            } else {
                                resend = true;
                                eprintln!("Failed to receive block ");
                            }
                        } else {
                            resend = false;
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

pub fn download_file(
    tracker: Arc<Mutex<TcpStream>>,
    file_size: u32,
    filename: String,
    p_to_c: HashMap<u32, HashSet<IpAddr>>,
    // local_ip: String,
) {
    dbg!(&p_to_c);
    let nblocks = p_to_c.len();
    let data_unsafe: Shared = Shared::new(filename.clone(), p_to_c);
    let nthreads: usize = if data_unsafe.peer_count < MAX_LEECH_THREADS as usize
    {
        data_unsafe.peer_count
    } else {
        MAX_LEECH_THREADS as usize
    };
    let max_chunks_id: u32 = data_unsafe.peers_to_chunk.len() as u32;

    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();
    dbg!(&nthreads, &data_unsafe.peer_count);

    let data: Arc<RwLock<Shared>> = Arc::new(RwLock::new(data_unsafe));
    let chunks_received: Arc<RwLock<HashSet<u32>>> =
        Arc::new(RwLock::new(HashSet::new()));
    let failed_chunks: Arc<RwLock<HashSet<u32>>> =
        Arc::new(RwLock::new(HashSet::new()));

    // DEBUG
    let n_blocks = if file_size % MAX_CHUNK_SIZE as u32 == 0 {
        file_size / MAX_CHUNK_SIZE as u32
    } else {
        file_size / MAX_CHUNK_SIZE as u32 + 1
    };
    assert_eq!(n_blocks, nblocks as u32);

    let mut file = Arc::new(
        create_part_file(filename.as_str(), file_size, nblocks as u32).unwrap(),
    );
    for _ in 0..nthreads {
        let t_handler = spawn(
            tracker.clone(),
            Arc::clone(&data),
            Arc::clone(&chunks_received),
            Arc::clone(&failed_chunks),
            max_chunks_id.clone(),
            Arc::clone(&mut file),
        );

        handles.push(t_handler);
    }

    for t in handles {
        t.join().unwrap();
    }

    complete_part_file(
        (filename + ".part").as_str(),
        file_size,
        nblocks as u32,
    )
    .unwrap();
}

fn spawn(
    tracker: Arc<Mutex<TcpStream>>,
    data: Arc<RwLock<Shared>>,
    chunks_received: Arc<RwLock<HashSet<u32>>>,
    chunks_failed: Arc<RwLock<HashSet<u32>>>,
    max_id: u32,
    mut file: Arc<File>,
) -> thread::JoinHandle<()> {
    let t_handler = thread::spawn(move || {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            loop {
                match stop_wait(&tracker, &socket, &data, &mut file) {
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

fn update_peer_latency(
    new_latency_ms: u16,
    ip: &IpAddr,
    data_rwl: &Arc<RwLock<Shared>>,
) {
    if let Ok(mut data) = data_rwl.write() {
        data.peers_latency.insert(ip.clone(), new_latency_ms);
    }
}

fn update_tracker_chunks(
    packet: &Protocol,
    filename: &String,
    tracker: &Arc<Mutex<TcpStream>>,
) -> anyhow::Result<()> {
    let mut buff = [0; 33];
    let b_chunk_id = packet.chunk_id.to_le_bytes();
    let b_fn_size = (filename.len() as u32).to_le_bytes();
    let b_filename = filename.as_bytes();
    buff[0..4].copy_from_slice(&b_chunk_id);
    buff[4..8].copy_from_slice(&b_fn_size);
    buff[8..8 + filename.len()].copy_from_slice(b_filename);
    let data_size = 8 + filename.len();
    let msg = FstpMessage {
        header: FstpHeader {
            flag: Flag::AddBlock,
            data_size: data_size as u16,
        },
        data: Some(&buff[..data_size]),
    };
    let mut msg_buff = [0u8; 200];
    let msg_size = msg.as_bytes(&mut msg_buff).unwrap();
    if let Ok(mut stream) = tracker.lock() {
        stream.write_all(&msg_buff[..msg_size])?;
    }
    Ok(())
}
