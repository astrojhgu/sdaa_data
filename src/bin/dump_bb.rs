use lockfree_object_pool::LinearOwnedReusable;
use std::{fs::File, io::Write, net::UdpSocket};

use clap::Parser;
use crossbeam::channel::unbounded;
use sdaa_data::{payload::Payload, pipeline::recv_pkt, utils::{as_u8_slice, set_recv_buffer_size}};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', long = "addr", value_name = "ip:port")]
    local_addr: String,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'p', value_name = "npkts to dump")]
    npkts_to_recv: Option<usize>,
}

fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();

    let socket = UdpSocket::bind(&args.local_addr).expect("failed to bind local addr");
    set_recv_buffer_size(&socket, 10*1024*1024*1024).unwrap();
    //let (tx, rx) = bounded::<LinearOwnedReusable<Payload>>(65536);
    let (tx, rx) = unbounded::<LinearOwnedReusable<Payload>>();
    let (_tx_cmd, rx_cmd)=unbounded();
    //let pool1 = Arc::clone(&pool);
    std::thread::spawn(|| recv_pkt(socket, tx, rx_cmd));

    let mut dump_file = args.outname.as_ref().map(|n| File::create(n).expect("failed to create dump file"));
    let mut npkts_received = 0;

    loop {
        let payload = rx.recv().expect("failed to recv payload");

        if payload.pkt_cnt % 100000 == 0 {
            println!("cnt: {} queue cnt: {}", payload.pkt_cnt, rx.len());
        }

        dump_file.as_mut().map(|f| {
            f.write_all(as_u8_slice(&payload.data)).expect("failed to write to dump file");
        });

        npkts_received += 1;
        if let Some(n) = args.npkts_to_recv {
            if npkts_received >= n {
                break;
            }
        }
    }
}
