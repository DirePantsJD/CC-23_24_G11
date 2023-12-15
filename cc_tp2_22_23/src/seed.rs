use std::sync::{Arc, Mutex, mpsc};
use std::net::{UdpSocket, SocketAddr, IpAddr, Ipv4Addr};
use std::io;
use std::thread;

use crate::fsnp;

const MAX_NUMBER_THREADS:usize = 2;

pub fn upload() -> io::Result<()>{
    let server_socket:Arc<UdpSocket> = Arc::new(UdpSocket::bind("0.0.0.0:".to_owned()+&fsnp::SERVER_UDP_PORT.to_string())?); 
    let send:Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
    let mut packets:Arc<Mutex<Vec<([u8;1500],usize,SocketAddr)>>> = Arc::new(Mutex::new(Vec::new()));
    let mut channels:Vec<mpsc::Sender<bool>> = Vec::new();
    let mut buffer:[u8;1500] = [0;1500];

    // create channel for each worker thread and spawn them
    for _ in 0..MAX_NUMBER_THREADS{
        // clone arcs for thread
        let ch:(mpsc::Sender<bool>,mpsc::Receiver<bool>) = mpsc::channel();
        let packets_clone = Arc::clone(&packets);
        let send_clone = Arc::clone(&send);
        let ssocket_clone = Arc::clone(&server_socket);
        
        thread::spawn(move||worker(packets_clone,ch.1,send_clone,ssocket_clone));
        
        channels.push(ch.0);
    }

    // listen -> push to buffer -> signal workers
    loop{
        let (bytes_read,src_addr) = server_socket.recv_from(&mut buffer).unwrap();

        if let Ok(mut acquired) = packets.lock(){
            acquired.push( (buffer.clone(),bytes_read as usize,src_addr) );
        }

        //signal worker threads
        channels.iter().for_each(|sender|sender.send(true).unwrap());
    }
}

fn worker(
    packets:Arc<Mutex<Vec<([u8;1500],usize,SocketAddr)>>>,
    recvch:mpsc::Receiver<bool>,
    send:Arc<Mutex<bool>>,
    server_socket:Arc<UdpSocket>
){
    let mut request_raw:([u8;1500],usize,SocketAddr) = ( 
        [0;1500],
        0,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127,0,0,1)),666)
    );
    let mut work:bool = false;
    let mut chunk_data:[u8;fsnp::MAX_CHUNK_SIZE] = [0;fsnp::MAX_CHUNK_SIZE];
    
    loop{
        // listen for signal
        if recvch.recv().unwrap(){
            // try lock -> pop request from queue
            if let Ok(mut acquired) = packets.try_lock(){
                if acquired.len()>0{
                    request_raw = acquired.remove(1);
                    work = true;
                }
            }
            if work{
                if let Ok(mut request) = fsnp::Protocol::read_packet(&request_raw.0,request_raw.1 as u16){
                    let bytes_read = read_chunk(request.filename,request.chunk_id,&mut chunk_data);
                    if bytes_read>0{
                        request.len_chunk = bytes_read as u16;
                        request.chunk_data = chunk_data.clone();
                        if let Some((packet,len)) = request.build_packet(){
                            // lock to prevent UdpSocket::send race
                            if let Ok(_) = send.lock(){
                                server_socket.send_to(&packet[..len as usize],request_raw.2);
                            }
                        }
                    }
                }
                work = false;
            }
        }
    }
}


//PLACEHOLDER
fn read_chunk(filename:&str,chunk_id:u32,buffer:&mut [u8]) -> usize {
    todo!()
}
