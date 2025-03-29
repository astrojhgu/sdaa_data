use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use num::Complex;
use std::{
    fs::File,
    io::Write,
    net::UdpSocket,
    sync::{Arc, Mutex},
};

use clap::Parser;
use crossbeam::channel::bounded;
use sdaa_data::{ddc::DownConverter, payload::Payload, pipeline::recv_pkt, utils::as_u8_slice};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', long = "addr", value_name = "ip:port")]
    local_addr: String,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'n', value_name = "npkts_per_dump")]
    npkt_per_dump: usize,

    #[clap(short = 'm', value_name = "dumps per npkt", default_value("100000"))]
    dump_per_npkt: usize,
}

fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();

    let socket = UdpSocket::bind(&args.local_addr).unwrap();
    let (tx_raw, rx_raw) = bounded::<LinearOwnedReusable<Payload>>(1024);

    let buf_cnt = Arc::new(Mutex::new(0));
    let buf_cnt1 = Arc::clone(&buf_cnt);
    let pool = Arc::new(LinearObjectPool::new(
        move || {
            eprint!(".");
            let mut cnt = buf_cnt1.lock().unwrap();
            *cnt += 1;
            Payload::default()
        },
        |v| {
            v.pkt_cnt = 0;
            v.data.fill(0);
        },
    ));

    let pool1 = Arc::clone(&pool);
    std::thread::spawn(|| recv_pkt(socket, tx_raw, pool1));

    let (tx_ddc, rx_ddc) = bounded(1024);

    let pool_ddc = Arc::new(LinearObjectPool::new(
        move || {
            eprintln!("#");
            Vec::new()
        },
        |_v| {},
    ));

    std::thread::spawn(move || {
        let mut ddc = DownConverter::new(32, 0.8, 5.0, 4, 800);
        ddc.process(rx_raw, tx_ddc, pool_ddc);
    });

    let mut outfile=args.outname.map(|n| File::create(n).unwrap());

    for i in 0.. {
        let ddc_data = rx_ddc.recv().unwrap();

        if i%10000==0{
            println!("{}", ddc_data.len());
        }

        if let Some(ref mut f)=outfile{
            
            let d=unsafe{std::slice::from_raw_parts(ddc_data.as_ptr() as *const u8, ddc_data.len()*std::mem::size_of::<Complex<f32>>())};
            f.write_all(d).unwrap();
            
        }
    }
}
