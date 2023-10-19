use anyhow::Context;
use fstp::*;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn main() -> anyhow::Result<()> {
    let _tracking_map: HashMap<String, BTreeMap<u64, String>> = HashMap::new();

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

fn handler(stream: &mut TcpStream) -> anyhow::Result<()> {
    let mut buffer = [0 as u8; 50];
    while stream.read(&mut buffer)? == 0 {}
    let msg = FstpMessage::from_bytes(&buffer)?;
    println!("{:?}", &msg);

    match msg.header.flag {
        Flag::Ok => {} //Em principio não deve de acontecer
        Flag::Add => {}
        Flag::List => {}
        Flag::File => {}
        Flag::Start => {}
        Flag::End => {}
        Flag::Exit => {}
    }

    stream
        .write("Hey *blushes*".as_bytes())
        .context("failed to write")?;

    Ok(())
}
