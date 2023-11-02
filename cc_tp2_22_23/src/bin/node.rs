#![feature(let_chains)]

use anyhow::{Context, bail};
use local::fstp::*;
use std::env;
use std::fs::{read_dir, File, ReadDir};
use std::io::{Read, Write, stdin,stdout};
use std::net::{TcpStream, IpAddr, Ipv4Addr, Shutdown};
use std::str::from_utf8;

fn main() -> anyhow::Result<()> {
    let mut stream = if let Some(tracker_addr) = env::args().nth(1){
        TcpStream::connect(tracker_addr)    
        .context("Can't connect to server")?
    } else {
        bail!("No tracker address specified (ip:port)")
    };

    contact_tracker(&mut stream)?;

    main_loop(&mut stream)?;

    Ok(())
}

fn main_loop(stream:&mut TcpStream) -> anyhow::Result<()> {
    let mut files: Vec<String> = Vec::new();
    let mut file_peers = PeersWithFile::new(); 
    loop {
        let mut buf = [0u8;1000];
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

                if stream.read(&mut buf)? == 0 {
                    bail!("Tracker no longer reachable");
                } 
                
                let response = FstpMessage::from_bytes(&buf)?;
                println!("resp:{:?}",response);
                if let Some(data) = response.data {
                    let list = from_utf8(data).unwrap().split(|c| c == ',');
                    for f in list {
                        files.push(String::from(f));
                    }
                }
                println!("files:{:#?}",files);
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

                if stream.read(&mut buf)? == 0 {
                    bail!("Server no longer reachable");
                } 
                
                let resp = FstpMessage::from_bytes(&buf)?;
                println!("resp:{:?}",resp);
                if let Some(data) = resp.data {
                    file_peers.set_name(&f_name.trim_end());
                    file_peers.set_peers_from_bytes(data)?;
                }
                println!("{:?}",file_peers);
            }
            "exit" => {
                stream.shutdown(Shutdown::Both)?;
                break;
            }
            _=> println!("Invalid command: {}",command),
        }
    }
    Ok(())
}

fn contact_tracker(stream:&mut TcpStream) ->anyhow::Result<()> {
    let shared_files = get_shared_files();
    let mut data_buffer = [0u8;1000];
    println!("shared files:\n{}",shared_files);
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
        read_dir(shared_path.trim_end()).expect(&format!("failed to read directory: {}",shared_path));

    for try_entry in shared_dir {
        let entry = try_entry.expect("failed to read entry");
        let path = entry.path();

        if path.is_file() 
        // && let Some(ext) = path.extension().and_then(|os_ext| os_ext.to_str()) 
        && let Some(name) = path.file_name().and_then(|os_str| os_str.to_str())
        // && ext == "gush"
         {
           shared_files.push_str(&(name.to_owned() + ","));
        }
    }
    shared_files.pop();
    shared_files
}

#[derive(Debug)]
struct PeersWithFile {
    name: String,
    peers: Vec<IpAddr>,
}

impl PeersWithFile {
    pub fn new() -> Self {
        PeersWithFile {
            name: String::new(),
            peers: Vec::new(),
        }
    }
     pub fn set_name(&mut self,str:&str) {
        self.name.push_str(str);
    }

    pub fn set_peers_from_bytes(&mut self,bytes: &[u8]) -> anyhow::Result<()>{
        let peers = &mut self.peers;
        let len = bytes.len();
        println!("{}",len);
        let max_iters = len/4;
        if len % 4 == 0 {
            for i in 0..max_iters {
                let idx = i*4;
                let b1 = bytes[idx];
                let b2 = bytes[idx + 1];
                let b3 = bytes[idx + 2];
                let b4 = bytes[idx + 3]; 
                let peer = IpAddr::V4(Ipv4Addr::new(b1,b2,b3,b4));  
                peers.push(peer);
            }
        } else {
            bail!("Corrupted addresses")
        }
        Ok(())
    }
}
