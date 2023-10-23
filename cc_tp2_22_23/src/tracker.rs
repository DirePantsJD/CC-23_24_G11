use anyhow::Context;
use fstp::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut tracking: HashMap<IpAddr, Vec<String>> = HashMap::new();

    let tcp_listener =
        TcpListener::bind("127.0.0.1:9090").context("binding failed")?;

    for mut stream in tcp_listener.incoming() {
        println!("new connection");
        match &mut stream {
            Ok(stream) => handler(stream, &mut tracking)?,
            Err(addr) => println!("Couldn't connect to {}", addr),
        }
    }

    Ok(())
}

// Pega na conexao
fn handler(
    stream: &mut TcpStream,
    tracking: &mut HashMap<IpAddr, Vec<String>>,
) -> anyhow::Result<()> {
    loop {
        // let mut buffer = Vec::with_capacity(100);
        let mut buffer = [0u8; 100];
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
                        println!("vec:{:?}", names_vec);
                        if !names_vec.iter().any(|s| s == file_name) {
                            tracking
                                .get_mut(&ip)
                                .unwrap()
                                .push(file_name.to_string());
                        }
                    }
                }
                println!("{:?}", tracking);
            }
            Flag::List => {
                let data: String = tracking
                    .values()
                    .cloned()
                    .flat_map(|vec| vec.into_iter())
                    .collect();
                println!("list:{:?}", data);
                let list_msg = FstpMessage {
                    header: FstpHeader { flag: Flag::Ok },
                    data: Some(data.as_bytes()),
                };

                // list_msg.put_in_bytes(&mut buffer)?;
                // stream.write_all(&mut buffer)?;
            }
            Flag::File => {}
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
