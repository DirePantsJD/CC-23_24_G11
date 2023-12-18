#![feature(ip_bits)]
use anyhow::{bail, Context};
use local::file_meta::FileMeta;
use local::fstp::*;
use local::peers_with_blocks::PeersWithFile;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::from_utf8;
use std::sync::{Arc, RwLock};
use threadpool::ThreadPool;

fn main() -> anyhow::Result<()> {
    let tracking_lock: Arc<RwLock<HashMap<IpAddr, Vec<FileMeta>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let file_to_ip_lock: Arc<RwLock<HashMap<String, Vec<(IpAddr, FileMeta)>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let tcp_listener = if let Some(listening_addr) = env::args().nth(1) {
        TcpListener::bind(listening_addr).context("binding failed")?
    } else {
        bail!("No tracker address specified (ip:port)");
    };

    let t_pool = ThreadPool::new(4);

    for stream in tcp_listener.incoming() {
        println!("new connection");
        match stream {
            Ok(stream) => {
                let tracking_lock_clone = tracking_lock.clone();
                let file_to_ip_lock_clone = file_to_ip_lock.clone();
                t_pool.execute(move || {
                    let ip = stream.peer_addr().unwrap().ip();
                    if let Ok(_) = handler(
                        stream,
                        tracking_lock_clone,
                        file_to_ip_lock_clone,
                    ) {
                        println!("{} connection closed", ip)
                    };
                })
            }
            Err(addr) => println!("Couldn't connect to {}", addr),
        }
    }

    Ok(())
}

// Pega na conexao
fn handler(
    mut stream: TcpStream,
    tracking_lock: Arc<RwLock<HashMap<IpAddr, Vec<FileMeta>>>>,
    file_to_ips_lock: Arc<RwLock<HashMap<String, Vec<(IpAddr, FileMeta)>>>>,
) -> anyhow::Result<()> {
    let mut buffer = [0u8; 1000];
    loop {
        // Se o stream TCP for fechado
        if stream.read(&mut buffer)? == 0 {
            let peer_ip = &stream.peer_addr()?.ip();
            if let Ok(mut tracking) = tracking_lock.write() {
                tracking.remove(peer_ip);
                if let Ok(mut file_to_ips) = file_to_ips_lock.write() {
                    for ips in file_to_ips.values_mut() {
                        if ips.iter().any(|(ip, _)| ip == peer_ip) {
                            if let Some(pos) =
                                ips.iter().position(|(ip, _)| ip == peer_ip)
                            {
                                ips.remove(pos);
                            }
                        }
                    }
                }
            }
            return Ok(());
        }
        let b = buffer.clone();
        let msg = FstpMessage::from_bytes(&b)?;
        println!("{:?}\n", &msg);

        match msg.header.flag {
            Flag::Add => {
                add(&mut stream, &tracking_lock, &file_to_ips_lock, msg)
            }
            Flag::AddBlock => {
                add_block(
                    stream.peer_addr().unwrap().ip(),
                    &tracking_lock,
                    &file_to_ips_lock,
                    msg,
                );
            }
            Flag::List => list(&mut stream, &tracking_lock, &mut buffer)?,
            Flag::File => {
                file(&mut stream, &file_to_ips_lock, msg, &mut buffer)?
            }
            Flag::Ok => {} //Em principio não deve de acontecer
        }
    }
}

fn add(
    stream: &mut TcpStream,
    tracking_lock: &Arc<RwLock<HashMap<IpAddr, Vec<FileMeta>>>>,
    file_to_ips_lock: &Arc<RwLock<HashMap<String, Vec<(IpAddr, FileMeta)>>>>,
    msg: FstpMessage,
) {
    if let Some(data) = msg.data {
        let mut files_meta = Vec::new();
        let mut iter = (0..data.len()).into_iter();

        while let Some(i) = iter.next() {
            if let Ok((fm_s, fm)) = FileMeta::from_bytes(&data[i..]) {
                files_meta.push(fm);
                iter.nth(fm_s - 2);
            } else {
                break;
            }
        }

        let ip = stream.peer_addr().unwrap().ip();
        //adiciona <ip,Vec> se n existir
        if let Ok(mut tracking) = tracking_lock.write() {
            if !tracking.contains_key(&ip) {
                tracking.insert(ip, Vec::new());
            }
            //Associa os metadados dos ficheiros no map de tracking
            //ao ip do cliente no stream
            for file_meta in files_meta {
                let file_name = file_meta.name.clone();
                let fs_m_vec = tracking.get(&ip).unwrap();

                if !fs_m_vec.iter().any(|fm| *fm.name == file_name) {
                    tracking.get_mut(&ip).unwrap().push(file_meta.clone());
                }
                if let Ok(mut file_to_ips) = file_to_ips_lock.write() {
                    if !file_to_ips.contains_key(&file_name) {
                        file_to_ips.insert(
                            String::from(&file_name),
                            vec![(ip, file_meta)],
                        );
                    } else {
                        let val = file_to_ips.get_mut(&file_name).unwrap();
                        if !val.iter().any(|(i, _)| i == &ip) {
                            val.push((ip, file_meta));
                        }
                    }
                }
            }
        }
    }
    // println!("{:?}", tracking_lock);
    // println!("{:?}", file_to_ips_lock);
}

fn add_block(
    ip: IpAddr,
    tracking_lock: &Arc<RwLock<HashMap<IpAddr, Vec<FileMeta>>>>,
    file_to_ips_lock: &Arc<RwLock<HashMap<String, Vec<(IpAddr, FileMeta)>>>>,
    msg: FstpMessage,
) {
    let data = msg.data.unwrap();
    let chunk_id = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let fn_size = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let file_name = from_utf8(&data[8..8 + fn_size as usize]).unwrap();
    if let Ok(mut tracking) = tracking_lock.write() {
        if let Some(files_m) = tracking.get_mut(&ip) {
            for fm in files_m.iter_mut() {
                if fm.name == file_name && !fm.has_full_file {
                    fm.blocks[chunk_id as usize] = 1;
                }
            }
        }
    }
    if let Ok(mut f_to_ips) = file_to_ips_lock.write() {
        if let Some(vals) = f_to_ips.get_mut(file_name) {
            for (peer_ip, fm) in vals.iter_mut() {
                if *peer_ip == ip && !fm.has_full_file {
                    fm.blocks[chunk_id as usize] = 1;
                }
            }
        }
    }
}

fn list(
    stream: &mut TcpStream,
    tracking_lock: &Arc<RwLock<HashMap<IpAddr, Vec<FileMeta>>>>,
    buffer: &mut [u8],
) -> anyhow::Result<()> {
    let mut data: String = String::new();
    if let Ok(tracking) = tracking_lock.write() {
        let uniq_vs: HashSet<FileMeta> =
            tracking.values().flatten().cloned().collect::<HashSet<_>>();
        for fm in uniq_vs {
            data.push_str(&(fm.name + ","));
        }
        data.pop();
    }
    println!("list:{:?}", data);
    let list_msg = FstpMessage {
        header: FstpHeader {
            flag: Flag::Ok,
            data_size: data.len() as u16,
        },
        data: Some(data.as_bytes()),
    };

    let l_msg_size = list_msg.as_bytes(buffer)?;
    stream.write_all(&buffer[..l_msg_size])?;
    stream.flush()?;
    Ok(())
}

fn file(
    stream: &mut TcpStream,
    file_to_ips_lock: &Arc<RwLock<HashMap<String, Vec<(IpAddr, FileMeta)>>>>,
    msg: FstpMessage,
    buffer: &mut [u8],
) -> anyhow::Result<()> {
    let mut file_size = 0;
    if let Some(data) = msg.data {
        let file_name = from_utf8(data).unwrap().trim_end();
        println!("Requested file: {}", file_name);

        let mut ips = vec![];
        let mut peers_with_file = HashSet::new();
        let mut peers_with_blocks = HashMap::new();
        if let Ok(file_to_ips) = file_to_ips_lock.read() {
            ips = file_to_ips.get(file_name).unwrap().clone();
            file_size = ips[0].1.f_size as u32;
            //^ seria melhor responder com "404" caso None ^
        }
        let n_blocks = {
            let (_, fm) = ips.first().unwrap();
            fm.blocks.len() as u32
        };
        for (ip, meta) in ips {
            if meta.has_full_file {
                peers_with_file.insert(ip);
            } else {
                for (b_id, val) in meta.blocks.iter().enumerate() {
                    let b_id = b_id as u32;
                    if *val == 1 {
                        if let None = peers_with_blocks.get(&b_id) {
                            let mut addrs = HashSet::new();
                            addrs.insert(ip);
                            peers_with_blocks.insert(b_id, addrs);
                        }
                    }
                }
            }
        }

        let peers_with_file = PeersWithFile {
            file_size,
            n_blocks,
            peers_with_file,
            peers_with_blocks,
        };

        let mut p_w_f_buf = [0u8; 1400];
        let size_p_w_f = peers_with_file.to_bytes(&mut p_w_f_buf);

        println!(
            "bytes:{:?}\n {:?}",
            &p_w_f_buf[..size_p_w_f as usize],
            size_p_w_f
        );

        let resp = FstpMessage {
            header: FstpHeader {
                flag: Flag::Ok,
                data_size: size_p_w_f,
            },
            data: Some(&p_w_f_buf[..size_p_w_f as usize]),
        };
        println!("Pre send:{:?}", resp);
        let resp_size = resp.as_bytes(buffer)?;
        stream.write_all(&buffer[..resp_size])?;
        stream.flush()?;
    }
    Ok(())
}
