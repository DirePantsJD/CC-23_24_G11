use std::env;
use std::fs;
use std::io::{Write, Read};
use std::net::UdpSocket;
use std::fs::File;
use std::os::unix::prelude::FileExt;
use anyhow::bail;
use protocol::*;

const CHUNK_BYTES:usize = 1420;

// reads the file and returns a tuple (chunk data bytes,length of data bytes) containing chunk data
fn read_file_chunk(filepath:String,chunk_id:u32) -> anyhow::Result<([u8;CHUNK_BYTES],usize)>{
    let mut data:[u8;CHUNK_BYTES] = [0;CHUNK_BYTES];
    let offset:u64 = chunk_id as u64*CHUNK_BYTES as u64;
    let file:File;
    
    match File::open(filepath.clone()){
        Ok(f) => file = f,
        Err(_) => match File::open(filepath+"/"+&chunk_id.to_string()){
            Ok(f) => file = f,
            Err(_) => bail!("No file or chunk found"),
        },
    }
    
    let bytes_read = file.read_at(&mut data,offset)?;

    return Ok((data,bytes_read));
}

fn write_file_chunk(filepath:String,chunk_id:u32,data:[u8;CHUNK_BYTES],length_data:u8) -> anyhow::Result<()>{
    match File::open(filepath.clone()+"/"+&chunk_id.to_string()){
        Ok(_) => bail!("Chunk ".to_owned() + &chunk_id.to_string() + " for " + &filepath + " already exists!"),
        Err(_) =>{
            let mut file = File::create(&chunk_id.to_string())?;
            file.write_all(&data[..length_data as usize])?;
            Ok(())
        },
    }    
}

fn merge_chunks(filepath:String) -> anyhow::Result<()>{
    let mut id = 0;
    let mut file:File;
    let mut buffer:[u8;CHUNK_BYTES] = [0;CHUNK_BYTES];
    
    match filepath.split("/").collect::<Vec<&str>>().last(){
        Some(filename) => file = File::create(filename)?,
        None => bail!("Failed generating master file name!"),
    }

    loop{
        let chunk_path = filepath.clone()+&id.to_string();
        let mut chunk_file = File::open(&chunk_path)?;
        let bytes_read = chunk_file.read(&mut buffer)?;
        file.write_all(&buffer[..bytes_read])?;
        fs::remove_file(&chunk_path)?;
        id+=1;
    }
}

fn listening_udp(socket:UdpSocket) -> anyhow::Result<()>{
    let mut packet:[u8;1500] = [0;1500];
    loop{
        let (bytes_read,src_ip) = socket.recv_from(&mut packet)?;
        let payload:protocol::FSNode_Protocol = protocol::FSNode_Protocol::read_packet(&packet)?;

    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let local_ip = if args.is_empty() { "127.0.0.1" } else { &args[1] };
    let socket = UdpSocket::bind(local_ip).unwrap();
    
}
