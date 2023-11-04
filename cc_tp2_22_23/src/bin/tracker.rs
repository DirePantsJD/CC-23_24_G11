#![feature(ip_bits)]
use anyhow::{bail, Context};
use local::file_meta::FileMeta;
use local::fstp::*;
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
    let file_to_ip_lock: Arc<RwLock<HashMap<String, Vec<IpAddr>>>> =
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
    file_to_ips_lock: Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
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
                        if ips.contains(peer_ip) {
                            if let Some(pos) =
                                ips.iter().position(|ip| ip == peer_ip)
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
    file_to_ips_lock: &Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
    msg: FstpMessage,
) {
    if let Some(data) = msg.data {
        let mut files_meta = Vec::new();
        let mut iter = (0..data.len()).into_iter();

        while let Some(i) = iter.next() {
            let fm_len = data[i] as usize;
            let fm = FileMeta::from_bytes(&data[i + 1..i + 1 + fm_len]);
            files_meta.push(fm);
            iter.nth(fm_len - 2);
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
                    tracking.get_mut(&ip).unwrap().push(file_meta);
                }
                if let Ok(mut file_to_ips) = file_to_ips_lock.write() {
                    if !file_to_ips.contains_key(&file_name) {
                        file_to_ips.insert(String::from(&file_name), vec![ip]);
                    }
                    let val = file_to_ips.get_mut(&file_name).unwrap();
                    if !val.contains(&ip) {
                        val.push(ip);
                    }
                }
            }
        }
    }
    println!("{:?}", tracking_lock);
    println!("{:?}", file_to_ips_lock);
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

    list_msg.put_in_bytes(buffer)?;
    stream.write_all(buffer)?;
    stream.flush()?;
    Ok(())
}

//TODO: Enviar metadados
fn file(
    stream: &mut TcpStream,
    file_to_ips_lock: &Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
    msg: FstpMessage,
    buffer: &mut [u8],
) -> anyhow::Result<()> {
    if let Some(data) = msg.data {
        let file = from_utf8(data).unwrap().trim_end();
        println!("Requested file: {}", file);
        let mut ips: Option<Vec<_>> = None;
        if let Ok(file_to_ips) = file_to_ips_lock.write() {
            ips = file_to_ips.get(file).cloned();
        }
        if let Some(mut ips) = ips {
            let ips_bytes: Vec<[u8; 4]> = ips
                .iter_mut()
                .map(|ip| match ip {
                    IpAddr::V4(ipv4) => ipv4.to_bits().to_be_bytes(),
                    _ => [0u8; 4],
                })
                .collect();
            let ips_bytes = ips_bytes.concat();
            let resp = FstpMessage {
                header: FstpHeader {
                    flag: Flag::Ok,
                    data_size: ips_bytes.len() as u16,
                },
                data: Some(ips_bytes.as_slice()),
            };
            println!("Pre send:{:?}", resp);
            resp.put_in_bytes(buffer)?;
            stream.write_all(buffer)?;
            stream.flush()?;
        }
    }
    Ok(())
}
