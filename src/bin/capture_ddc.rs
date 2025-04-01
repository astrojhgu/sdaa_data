use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    net::UdpSocket,
    sync::{Arc, Mutex},
};

use clap::Parser;
use crossbeam::channel::bounded;
use num::{traits::FloatConst, Complex};

#[cfg(feature = "cuda")]
use sdaa_data::{ddc::fir_coeffs2 as fir_coeffs, pipeline::pkt_ddc};

use sdaa_data::{
    fir::design_lowpass_filter,
    payload::{Payload, N_PT_PER_FRAME},
    pipeline::recv_pkt,
    utils::{as_u8_slice, slice_as_u8},
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', long = "addr", value_name = "ip:port")]
    local_addr: String,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'l', value_name = "loch")]
    lo_ch: isize,
}

#[cfg(feature = "cuda")]
fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();

    let socket = UdpSocket::bind(&args.local_addr).unwrap();
    let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(8192);
    let (tx_ddc, rx_ddc) = bounded::<LinearOwnedReusable<Vec<Complex<f32>>>>(8192);

    let lo_ch = args.lo_ch;
    //assert!(lo_ch>512 && lo_ch<1536);
    //let pool1 = Arc::clone(&pool);
    std::thread::spawn(|| recv_pkt(socket, tx_payload));
    std::thread::spawn(move || {
        let lo_cos: Vec<_> = (0..N_PT_PER_FRAME)
            .map(|i| ((i as isize * lo_ch) as f32 / N_PT_PER_FRAME as f32 * 2.0 * f32::PI()).cos())
            .collect();

        let lo_sin: Vec<_> = (0..N_PT_PER_FRAME)
            .map(|i| -((i as isize * lo_ch) as f32 / N_PT_PER_FRAME as f32 * 2.0 * f32::PI()).sin())
            .collect();

        let fir_coeffs = fir_coeffs();
        pkt_ddc(rx_payload, tx_ddc, 8, &lo_cos, &lo_sin, &fir_coeffs);
    });

    let mut bytes_written = 0;

    let mut dump_file = args.outname.map(|outname| File::create(&outname).unwrap());

    loop {
        let ddc = rx_ddc.recv().unwrap();

        if let Some(ref mut f) = dump_file {
            //dump_file = Some(File::create(outname).unwrap());
            f.write_all(slice_as_u8(&ddc[..])).unwrap();
            bytes_written += ddc.len() * std::mem::size_of::<Complex<f32>>();
            //println!("{} MBytes written", bytes_written as f64/1e6);
        }
    }
}

#[cfg(not(feature = "cuda"))]
fn main() {}
