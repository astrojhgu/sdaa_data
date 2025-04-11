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
use sdaa_data::{
    payload::Payload,
    pipeline::{pkt_fft, pkt_integrate, recv_pkt},
    utils::{as_u8_slice, slice_as_u8},
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', long = "addr", value_name = "ip:port")]
    local_addr: String,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'c', long = "nch", value_name = "num of ch")]
    nch: usize,

    #[clap(short = 'n', long = "nint", value_name = "num of fft per integration")]
    nint: usize,
}

fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();

    let socket = UdpSocket::bind(&args.local_addr).unwrap();
    let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(16384);
    let (tx_fft, rx_fft) = bounded::<LinearOwnedReusable<Vec<Complex<f32>>>>(1024);
    let (tx_wf, rx_wf) = bounded::<LinearOwnedReusable<Vec<f32>>>(4096);

    //let pool1 = Arc::clone(&pool);
    std::thread::spawn(|| recv_pkt(socket, tx_payload));
    std::thread::spawn(move || pkt_fft(rx_payload, tx_fft, args.nch));
    std::thread::spawn(move || pkt_integrate(rx_fft, tx_wf, args.nint));

    //let mut dump_file = None;
    let mut outfile = args.outname.map(|outname| File::create(&outname).unwrap());
    for i in 0.. {
        let x = rx_wf.recv().unwrap();
        outfile.iter_mut().for_each(|f| {
            f.write_all(slice_as_u8(&x[..])).unwrap();
        });
    }
}
