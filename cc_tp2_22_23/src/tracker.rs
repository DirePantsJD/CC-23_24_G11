#![feature(ip_bits)]
use anyhow::Context;
use fstp::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let tcp_listener =
        TcpListener::bind("127.0.0.1:9090").context("binding failed")?;

    for mut stream in tcp_listener.incoming() {
        println!("new connection");
        match &mut stream {
            Ok(stream) => handler(stream)?,
            Err(addr) => println!("Couldn't connect to {}", addr),
        }
    }

    Ok(())
}

// Pega na conexao
fn handler(stream: &mut TcpStream) -> anyhow::Result<()> {
    let mut tracking: HashMap<IpAddr, Vec<String>> = HashMap::new();
    let mut file_to_ip: HashMap<String, Vec<IpAddr>> = HashMap::new();
    let mut buffer = [0u8; 100];
    loop {
        //prob vai ter de reconhecer fim da comunicação ou dar timeout
        while stream.read(&mut buffer)? == 0 {}

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
                        if !file_to_ip.contains_key(file_name) {
                            file_to_ip.insert(
                                String::from(file_name),
                                vec![ip.clone()],
                            );
                        }
                        let val = file_to_ip.get_mut(file_name).unwrap();
                        if !val.contains(&ip) {
                            val.push(ip);
                        }
                    }
                }
                println!("{:?}", tracking);
                println!("{:?}", file_to_ip);
            }
            Flag::List => {
                let mut data: String = String::new();
                for (k, v) in &tracking {
                    data.push_str(&(k.to_string() + ":" + &v.join(",") + ";"));
                }
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
                    if let Some(mut ips) = file_to_ip.get(file).cloned() {
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
                            data: Some(&ips_bytes),
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
