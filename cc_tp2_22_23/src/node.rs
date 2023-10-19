#![feature(let_chains)]

use anyhow::Context;
use fstp::*;
use std::fs::{read_dir, File, ReadDir};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut buffer = [0u8; 50];

    let shared_files = get_shared_files();
    println!("shared files:\n{}",shared_files);

    let mut stream = TcpStream::connect("127.0.0.1:9090")
        .context("Can't connect to server")?;
    let msg = FstpMessage {
        header: FstpHeader { flag: Flag::Add },
        data: None,
    };

    msg.to_bytes(&mut buffer);

    stream.write(&buffer)?;

    while stream.read(&mut buffer)? == 0 {}

    println!("{:?}", from_utf8(&buffer)?.trim_matches(|c| c == '\0'));
    Ok(())
}

fn get_shared_files() -> String {
    let mut shared_files = String::new();
    let mut config = File::open("./node.config").expect("No config file found");

    let mut shared_path = String::new();
    config
        .read_to_string(&mut shared_path)
        .expect("Inválid path");

    let shared_dir: ReadDir =
        read_dir(shared_path).expect("failed to read directory");

    for try_entry in shared_dir {
        let entry = try_entry.expect("failed to read entry");
        let path = entry.path();

        if path.is_file() && let Some(os_ext) = path.extension() 
        && let Some(name) = path.file_name().and_then(|os_str| os_str.to_str())
        && let Some(ext) = os_ext.to_str()
        && ext == "gush" {
           shared_files.push_str(&(name.to_owned() + ";"));
        }
    }
    shared_files.pop();
    shared_files
}
