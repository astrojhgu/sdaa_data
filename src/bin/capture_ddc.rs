use std::{fs::File, io::Write};

use clap::Parser;
use num::Complex;

use sdaa_data::{ddc::npt_ddc_per_dump, sdr::Sdr, utils::slice_as_u8};

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

    #[clap(short = 'N', value_name = "Num of samples in 10^6")]
    nsamp: Option<usize>,

    #[clap(short = 'C')]
    ignore_locking: bool,
}

#[cfg(feature = "cuda")]
fn main() {
    //let (tx,rx)=bounded(256);

    use sdaa_data::pipeline::DdcCmd;

    let args = Args::parse();

    let (sdr, rx_ddc, tx_cmd) = Sdr::new(
        args.remote_ctrl_addr.parse().unwrap(),
        args.local_ctrl_addr.parse().unwrap(),
        args.local_payload_addr.parse().unwrap(),
    );

    let mut dump_file = args.outname.map(|outname| File::create(&outname).unwrap());
    let mut _bytes_written = 0;
    tx_cmd
        .send(sdaa_data::pipeline::DdcCmd::LoCh(args.lo_ch))
        .unwrap();
    sdr.wakeup();
    if !args.ignore_locking {
        sdr.wait_until_locked(60);
    } else {
        eprintln!("ignoring clock locking");
    }

    sdr.init();
    sdr.sync();
    sdr.stream_start();
    let mut nsamp: Option<usize> = args.nsamp.map(|x| x * 1_000_000);
    for _i in 0.. {
        let ddc = rx_ddc.recv().unwrap();

        let n_to_write = if let Some(n) = nsamp {
            n.min(ddc.len())
        } else {
            ddc.len()
        };

        if n_to_write == 0 {
            break;
        }

        nsamp.iter_mut().for_each(|x| {
            *x -= n_to_write;
        });
        if let Some(ref mut f) = dump_file {
            //dump_file = Some(File::create(outname).unwrap());
            f.write_all(slice_as_u8(&ddc[..n_to_write])).unwrap();
            _bytes_written += ddc.len() * std::mem::size_of::<Complex<f32>>();
            //println!("{} MBytes written", bytes_written as f64 / 1e6);
        }
    }
    tx_cmd.send(DdcCmd::Destroy).unwrap();
    drop(rx_ddc);
}

#[cfg(not(feature = "cuda"))]
fn main() {}
