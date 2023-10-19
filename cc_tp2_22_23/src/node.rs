use anyhow::Context;
use fstp::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let data = [0u8; 50];
    let mut buffer = [0u8; 50];
    let mut stream = TcpStream::connect("127.0.0.1:9090")
        .context("Can't connect to server")?;
    let msg = FstpMessage {
        header: fstp::FstpHeader { flag: Flag::Add },
        data: &data,
    };

    msg.to_bytes(&mut buffer);

    stream.write(&buffer)?;

    while stream.read(&mut buffer)? == 0 {}

    println!("{:?}", from_utf8(&buffer)?.trim_matches(|c| c == '\0'));
    Ok(())
}
