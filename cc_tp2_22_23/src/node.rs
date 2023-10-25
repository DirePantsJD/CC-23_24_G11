#![feature(let_chains)]

use anyhow::Context;
use fstp::*;
use std::collections::HashMap;
use std::fs::{read_dir, File, ReadDir};
use std::io::{Read, Write, stdin,stdout};
use std::net::{TcpStream, IpAddr};
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:9090")
        .context("Can't connect to server")?;

    contact_tracker(&mut stream)?;

    main_loop(&mut stream)?;

    Ok(())
}

fn main_loop(stream:&mut TcpStream) -> anyhow::Result<()> {
    let mut peers_files: HashMap<IpAddr,Vec<String>> = HashMap::new();
    loop {
        let mut buf = [0u8;100];
        let mut raw_command = String::new();
        stdout().write_all("Input command\n".as_bytes())?;
        stdout().flush()?;
        stdin().read_line(&mut raw_command)?;
        let command = String::from(raw_command.to_lowercase().trim_end());

        match command.as_str() {
            "list" => {
                let msg = FstpMessage{
                    header: FstpHeader { flag: Flag::List, data_size:0 },
                    data:None,
                };
                msg.put_in_bytes(&mut buf)?;
                stream.write_all(&buf)?;
                stream.flush()?;

                while stream.read(&mut buf)?==0 {}
                
                let response = FstpMessage::from_bytes(&buf)?;
                println!("resp:{:?}",response);
                if let Some(data) = response.data {
                    let entries = from_utf8(data).unwrap().split(|c| c == ';');
                    for entry in entries {
                       if let Some((ip,files)) = entry.split_once(':'){
                            let ip = ip.parse::<IpAddr>()?;
                            let files_v:Vec<String> = 
                                files.split(|c| c==',')
                                    .map(|str| String::from(str))
                                    .collect();
                            peers_files.insert(ip, files_v);
                        }
                    }
                }
                println!("files_map:{:#?}",peers_files);
            }
            "file" => {
                let mut f_name = String::new();
                stdout().write_all("Input file name\n".as_bytes())?;
                stdout().flush()?;
                stdin().read_line(&mut f_name)?;                
                let msg = FstpMessage {
                    header: FstpHeader { 
                        flag: Flag::File,
                        data_size:f_name.len() as u16
                    },
                    data: Some(f_name.as_bytes())
                };
                msg.put_in_bytes(&mut buf)?;
                stream.write(&mut buf)?;
                stream.flush()?;

                while stream.read(&mut buf)?==0 {}
                
                let response = FstpMessage::from_bytes(&buf)?;
                println!("resp:{:?}",response);
                //TODO: por info numa struct qualquer;
            }
            "exit" => {
                stream.shutdown(std::net::Shutdown::Both)?;
            }
            _=> println!("Invalid command: {}",command),
        }
    }
}

fn contact_tracker(stream:&mut TcpStream) ->anyhow::Result<()> {
    let shared_files = get_shared_files();
    let mut data_buffer = [0u8;100];
    println!("shared files:\n{}",shared_files);
    println!("sf:len{:?}",shared_files.len());
    let msg = FstpMessage {
        header: FstpHeader { 
            flag: Flag::Add,
            data_size: shared_files.len() as u16 
        },
        data: Some(shared_files.as_bytes()),
    };
  
    msg.put_in_bytes(&mut data_buffer)?;

    stream.write_all(&data_buffer)?;
    stream.flush()?;
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

        if path.is_file() 
        && let Some(ext) = path.extension().and_then(|os_ext| os_ext.to_str()) 
        && let Some(name) = path.file_name().and_then(|os_str| os_str.to_str())
        && ext == "gush" {
           shared_files.push_str(&(name.to_owned() + ","));
        }
    }
    shared_files.pop();
    shared_files
}
