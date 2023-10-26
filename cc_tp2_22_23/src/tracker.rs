#![feature(ip_bits)]
use anyhow::Context;
use fstp::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut tracking: HashMap<IpAddr, Vec<String>> = HashMap::new();
    let mut file_to_ip: HashMap<String, Vec<IpAddr>> = HashMap::new();
    let tcp_listener =
        TcpListener::bind("127.0.0.1:9090").context("binding failed")?;

    for mut stream in tcp_listener.incoming() {
        println!("new connection");
        match &mut stream {
            Ok(stream) => handler(stream, &mut tracking, &mut file_to_ip)?,
            Err(addr) => println!("Couldn't connect to {}", addr),
        }
    }

    Ok(())
}

// Pega na conexao
fn handler(
    stream: &mut TcpStream,
    tracking: &mut HashMap<IpAddr, Vec<String>>,
    file_to_ips: &mut HashMap<String, Vec<IpAddr>>,
) -> anyhow::Result<()> {
    let mut buffer = [0u8; 100];
    loop {
        // Se o stream TCP dor fechado
        if stream.read(&mut buffer)? == 0 {
            let peer_ip = &stream.peer_addr()?.ip();
            tracking.remove(peer_ip);
            for (_, ips) in file_to_ips.iter_mut() {
                if ips.contains(peer_ip) {
                    if let Some(pos) = ips.iter().position(|ip| ip == peer_ip) {
                        ips.remove(pos);
                    }
                }
            }
            return Ok(());
        }

        let msg = FstpMessage::from_bytes(&buffer)?;
        println!("{:?}\n", &msg);

        match msg.header.flag {
            Flag::Add => {
                if let Some(data) = msg.data {
                    let files = from_utf8(data).unwrap().split(|c| c == ',');

                    let ip = stream.peer_addr().unwrap().ip();
                    //adiciona <ip,Vec> se n existir
                    if !tracking.contains_key(&ip) {
                        tracking.insert(ip.clone(), Vec::new());
                    }
                    //Associa os nomes dos ficheiros no map de tracking
                    //ao ip do cliente no stream
                    for file_name in files {
                        let names_vec = tracking.get(&ip).unwrap();

                        if !names_vec.iter().any(|s| s == file_name) {
                            tracking
                                .get_mut(&ip)
                                .unwrap()
                                .push(file_name.to_string());
                        }
                        if !file_to_ips.contains_key(file_name) {
                            file_to_ips.insert(
                                String::from(file_name),
                                vec![ip.clone()],
                            );
                        }
                        let val = file_to_ips.get_mut(file_name).unwrap();
                        if !val.contains(&ip) {
                            val.push(ip);
                        }
                    }
                }
                println!("{:?}", tracking);
                println!("{:?}", file_to_ips);
            }
            Flag::List => {
                let mut data: String = String::new();
                for v in tracking.values() {
                    data.push_str(&(v.join(",") + ","));
                }
                data.pop();
                println!("list:{:?}", data);
                let list_msg = FstpMessage {
                    header: FstpHeader {
                        flag: Flag::Ok,
                        data_size: data.len() as u16,
                    },
                    data: Some(data.as_bytes()),
                };

                list_msg.put_in_bytes(&mut buffer)?;
                stream.write_all(&mut buffer)?;
                stream.flush()?;
            }
            Flag::File => {
                if let Some(data) = msg.data {
                    let file = from_utf8(data).unwrap().trim_end();
                    println!("Requested file: {}", file);
                    if let Some(mut ips) = file_to_ips.get(file).cloned() {
                        let ips_bytes: Vec<[u8; 4]> = ips
                            .iter_mut()
                            .map(|ip| match ip {
                                IpAddr::V4(ipv4) => {
                                    ipv4.to_bits().to_be_bytes()
                                }
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
                        resp.put_in_bytes(&mut buffer)?;
                        stream.write_all(&mut buffer)?;
                        stream.flush()?;
                    }
                }
            }
            // Flag::Exit => {
            // let ip = stream.peer_addr().unwrap().ip();
            // tracking.remove(&ip);
            //??
            //stream.shutdown(std::net::Shutdown::Both);
            // }
            Flag::Ok => {} //Em principio não deve de acontecer
        }
    }
}
