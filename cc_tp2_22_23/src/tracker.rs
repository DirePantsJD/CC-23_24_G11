use anyhow::Context;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

fn handler(stream: &mut TcpStream) -> anyhow::Result<()> {
    let mut buffer = [0 as u8; 50];
    while stream.read(&mut buffer)? == 0 {}

    println!("{:?}", from_utf8(&buffer)?.trim_matches(|c| c == '\0'));

    stream
        .write("Hey *blushes*".as_bytes())
        .context("failed to write")?;

    Ok(())
}
