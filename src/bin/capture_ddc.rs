use std::{fs::File, io::Write};

use clap::Parser;
use num::Complex;

use sdaa_data::{sdr::Sdr, utils::slice_as_u8};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', value_name = "local payload ip:port")]
    local_payload_addr: String,

    #[clap(short = 'A', value_name = "remote ctrl ip:port")]
    remote_ctrl_addr: String,

    #[clap(
        short = 'L',
        value_name = "local ctrl ip:port",
        default_value = "0.0.0.0:3001"
    )]
    local_ctrl_addr: String,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'l', value_name = "loch")]
    lo_ch: isize,
}

#[cfg(feature = "cuda")]
fn main() {
    //let (tx,rx)=bounded(256);

    let args = Args::parse();

    let (sdr, rx_ddc, tx_lo_ch) = Sdr::new(
        args.remote_ctrl_addr.parse().unwrap(),
        args.local_ctrl_addr.parse().unwrap(),
        args.local_payload_addr.parse().unwrap(),
    );

    let mut dump_file = args.outname.map(|outname| File::create(&outname).unwrap());
    let mut _bytes_written = 0;
    tx_lo_ch.send(args.lo_ch).unwrap();
    sdr.wakeup();
    sdr.wait_until_locked(60);
    sdr.init();
    sdr.sync();
    sdr.stream_start();
    for _i in 0..10 {
        let ddc = rx_ddc.recv().unwrap();

        if let Some(ref mut f) = dump_file {
            //dump_file = Some(File::create(outname).unwrap());
            f.write_all(slice_as_u8(&ddc[..])).unwrap();
            _bytes_written += ddc.len() * std::mem::size_of::<Complex<f32>>();
            //println!("{} MBytes written", bytes_written as f64 / 1e6);
        }
    }

    drop(rx_ddc);
}

#[cfg(not(feature = "cuda"))]
fn main() {}
