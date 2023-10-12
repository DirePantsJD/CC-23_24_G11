use anyhow::Context;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut buffer = [0 as u8; 50];
    let mut stream = TcpStream::connect("127.0.0.1:9090")
        .context("Can't connect to server")?;

    stream.write("Hey *winks*".as_bytes())?;

    while stream.read(&mut buffer)? == 0 {}

    println!("{:?}", from_utf8(&buffer)?.trim_matches(|c| c == '\0'));
    Ok(())
}
