#![feature(let_chains)]

use anyhow::{Context, bail};
use local::file_meta::*;
use local::seed::upload;
use local::fstp::*;
use local::peers_with_blocks::*;
use local::leech::download_file;
use bitvec::prelude::*;
use std::collections::HashSet;
use std::env;
use std::sync::{Mutex,Arc};
use std::fs::{read_dir, File, ReadDir};
use std::io::{Read, Write, stdin,stdout};
use std::net::{TcpStream,Shutdown};
use std::thread;
use std::str::from_utf8;

// const CHUNK_BYTES:u16 = 1420; 

fn main() -> anyhow::Result<()> {
    let stream = if let Some(tracker_addr) = env::args().nth(1){
        TcpStream::connect(tracker_addr)    
        .context("Can't connect to server")?
    } else {
        bail!("No tracker address specified (ip:port)")
    };
    let stream = Arc::new(Mutex::new(stream));

    contact_tracker(stream.clone())?;

    thread::spawn(move || upload());

    main_loop(&stream.clone())?;

    Ok(())
}

fn main_loop(stream:&Arc<Mutex<TcpStream>>) -> anyhow::Result<()> {
    let mut files: HashSet<String> = HashSet::new();
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
                let msg_size = msg.as_bytes(&mut buf)?;
                if let Ok(mut stream) = stream.lock() {
                    stream.write_all(&buf[..msg_size])?;
                    stream.flush()?;
                
                    if stream.read(&mut buf)? == 0 {
                        bail!("Tracker no longer reachable");
                    } 
                }
                
                let response = FstpMessage::from_bytes(&buf)?;
                println!("resp:{:?}",response);
                if let Some(data) = response.data {
                    let list = from_utf8(data).unwrap().split(|c| c == ',');
                    for f in list {
                        files.insert(String::from(f));
                    }
                }
                println!("files:{:#?}",files);
            }
            "file" => {
                let mut f_name = String::new();
                stdout().write_all("Input file name\n".as_bytes())?;
                stdout().flush()?;
                stdin().read_line(&mut f_name)?;                

                if files.iter().any(|str| str == &f_name.trim_end()) {
                    let msg = FstpMessage {
                        header: FstpHeader { 
                            flag: Flag::File,
                            data_size:f_name.len() as u16
                        },
                        data: Some(f_name.as_bytes())
                    };
                    let msg_size = msg.as_bytes(&mut buf)?;
                    if let Ok(mut stream) = stream.lock() { 
                        stream.write_all(&mut buf[..msg_size])?;
                        stream.flush()?;
    
                        if stream.read(&mut buf)? == 0 {
                            bail!("Server no longer reachable");
                        }
                    }
                
                    let resp = FstpMessage::from_bytes(&buf)?;
                    println!("resp:{:?}",resp);
                    if let Some(data) = resp.data {
                        let peers_with_file = PeersWithFile::from_bytes(data)?;
                        println!("p_w_f:{:?}",peers_with_file);
                        let mut p_to_cs = peers_with_file.peers_with_blocks.clone();
                        for k in peers_with_file.peers_with_blocks.keys() {
                            for p_w_f in peers_with_file.peers_with_file.iter() {
                                if let Some(val) = p_to_cs.get_mut(k){
                                    val.insert(*p_w_f);
                                }
                            }
                        }
                        download_file(stream.clone(),peers_with_file.file_size,f_name,peers_with_file.peers_with_blocks);

                    }
                }
            }
            "exit" => {
                if let Ok(stream) = stream.lock() {
                    stream.shutdown(Shutdown::Both)?;
                }
                break;
                
            }
            _=> println!("Invalid command: {}",command),
        }
    }
    Ok(())
}

fn contact_tracker(stream:Arc<Mutex<TcpStream>>) ->anyhow::Result<()> {
    let files_meta = get_files_meta();
    let mut fm_buf = [0u8;100];
    let mut raw_data = [0u8;1000];
    let mut data_size = 0;
    let mut prev_ds = 0;

    for f_m in files_meta {
        let fm_size = f_m.as_bytes(&mut fm_buf).expect("Failed to serialize FM");
        data_size+=fm_size;
        raw_data[prev_ds..data_size].copy_from_slice(&fm_buf[..fm_size]);
        prev_ds = data_size;
    }
    let msg = FstpMessage {
        header: FstpHeader { 
            flag: Flag::Add,
            data_size: data_size as u16 
        },
        data: Some(&raw_data.as_slice()[..data_size]) 
    };
    println!("{:#?}",msg);
  
    let mut msg_buffer = [0u8;2000];
    let msg_size = msg.as_bytes(&mut msg_buffer)?;

    if let Ok(mut stream) = stream.lock() {
        stream.write_all(&msg_buffer[..msg_size])?;
        stream.flush()?;
    }
    Ok(())
}

fn get_files_meta() -> Vec<FileMeta> {
    let mut files_meta = Vec::new();
    let mut config = File::open("./node.config").expect("No config file found");

    let mut shared_path = String::new();
    config
        .read_to_string(&mut shared_path)
        .expect("Inválid path");

    let shared_dir: ReadDir =
        read_dir(shared_path.trim_end()).expect(
            &format!("failed to read directory: {}",shared_path)
        );

    for try_entry in shared_dir {
        let entry = try_entry.expect("failed to read entry");
        let path = entry.path();

        if path.is_file() 
        && let Ok(meta) = entry.metadata() 
        && let Some(name) = path.file_name().and_then(|os_str| os_str.to_str())
        {
            let f_size = meta.len();
            let f_m = FileMeta {
                f_size,
                has_full_file: true,
                blocks_len: 0,
                name_len: name.len() as u16,
                blocks: BitVec::<u8,Msb0>::new(),
                name:name.to_string()
            };
            files_meta.push(f_m);
        }
    }
    files_meta
}

