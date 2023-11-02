#![feature(ip_bits)]
use anyhow::{bail, Context};
use local::fstp::*;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::from_utf8;
use std::sync::{Arc, RwLock};
use threadpool::ThreadPool;

fn main() -> anyhow::Result<()> {
    let tracking_lock: Arc<RwLock<HashMap<IpAddr, Vec<String>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let file_to_ip_lock: Arc<RwLock<HashMap<String, Vec<IpAddr>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let tcp_listener = if let Some(listening_addr) = env::args().nth(1) {
        TcpListener::bind(listening_addr).context("binding failed")?
    } else {
        bail!("No tracker address specified (ip:port)");
    };

    let t_pool = ThreadPool::new(5);

    for stream in tcp_listener.incoming() {
        println!("new connection");
        match stream {
            Ok(stream) => {
                let tracking = tracking_lock.clone();
                let file_to_ip = file_to_ip_lock.clone();
                t_pool.execute(move || {
                    let ip = stream.peer_addr().unwrap().ip();
                    if let Ok(_) = handler(stream, tracking, file_to_ip) {
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
    tracking: Arc<RwLock<HashMap<IpAddr, Vec<String>>>>,
    file_to_ips: Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
) -> anyhow::Result<()> {
    let mut buffer = [0u8; 1000];
    loop {
        // Se o stream TCP for fechado
        if stream.read(&mut buffer)? == 0 {
            let peer_ip = &stream.peer_addr()?.ip();
            if let Ok(mut tracking_w_guard) = tracking.write() {
                tracking_w_guard.remove(peer_ip);
                if let Ok(mut ftis_w_guard) = file_to_ips.write() {
                    for (_, ips) in ftis_w_guard.iter_mut() {
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
            Flag::Add => add(&mut stream, &tracking, &file_to_ips, msg),
            Flag::List => list(&mut stream, &tracking, &mut buffer)?,
            Flag::File => file(&mut stream, &file_to_ips, msg, &mut buffer)?,
            Flag::Ok => {} //Em principio não deve de acontecer
        }
    }
}

fn add(
    stream: &mut TcpStream,
    tracking: &Arc<RwLock<HashMap<IpAddr, Vec<String>>>>,
    file_to_ips: &Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
    msg: FstpMessage,
) {
    if let Some(data) = msg.data {
        let files = from_utf8(data).unwrap().split(|c| c == ',');

        let ip = stream.peer_addr().unwrap().ip();
        //adiciona <ip,Vec> se n existir
        if let Ok(mut tracking_w_guard) = tracking.write() {
            if !tracking_w_guard.contains_key(&ip) {
                tracking_w_guard.insert(ip.clone(), Vec::new());
            }
            //Associa os nomes dos ficheiros no map de tracking_w_guard
            //ao ip do cliente no stream
            for file_name in files {
                let names_vec = tracking_w_guard.get(&ip).unwrap();

                if !names_vec.iter().any(|s| s == file_name) {
                    tracking_w_guard
                        .get_mut(&ip)
                        .unwrap()
                        .push(file_name.to_string());
                }
                if let Ok(mut ftis_w_guard) = file_to_ips.write() {
                    if !ftis_w_guard.contains_key(file_name) {
                        ftis_w_guard
                            .insert(String::from(file_name), vec![ip.clone()]);
                    }
                    let val = ftis_w_guard.get_mut(file_name).unwrap();
                    if !val.contains(&ip) {
                        val.push(ip);
                    }
                }
            }
        }
    }
    println!("{:?}", tracking);
    println!("{:?}", file_to_ips);
}

fn list(
    stream: &mut TcpStream,
    tracking: &Arc<RwLock<HashMap<IpAddr, Vec<String>>>>,
    buffer: &mut [u8],
) -> anyhow::Result<()> {
    let mut data: String = String::new();
    if let Ok(tracking_w_guard) = tracking.write() {
        let uniq_vs: HashSet<String> = tracking_w_guard
            .values()
            .cloned()
            .flatten()
            .collect::<HashSet<_>>();
        for s in uniq_vs {
            data.push_str(&(s + ","));
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

fn file(
    stream: &mut TcpStream,
    file_to_ips: &Arc<RwLock<HashMap<String, Vec<IpAddr>>>>,
    msg: FstpMessage,
    buffer: &mut [u8],
) -> anyhow::Result<()> {
    if let Some(data) = msg.data {
        let file = from_utf8(data).unwrap().trim_end();
        println!("Requested file: {}", file);
        let mut ips: Option<Vec<_>> = None;
        if let Ok(ftis_w_guard) = file_to_ips.write() {
            ips = ftis_w_guard.get(file).cloned();
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
