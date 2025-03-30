use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use num::Complex;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    net::UdpSocket,
    slice,
    sync::{Arc, Mutex},
};

use chrono::Local;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use crossbeam::channel::bounded;
use sdaa_data::{
    ddc::ddc_x8,
    payload::{Payload, N_PT_PER_FRAME},
    pipeline::recv_pkt,
    utils::{as_u8_slice, slice_as_u8},
    Ftype, RAW_SAMP_RATE,
};

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

    #[clap(short = 'l', long="lo", value_name="lo freq in MHz", default_value("100.0"))]
    lo_megahz: f64
}

fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();

    let socket = UdpSocket::bind(&args.local_addr).unwrap();
    let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(4096);
    let (tx_ddc, rx_ddc) = bounded::<LinearOwnedReusable<Vec<Complex<Ftype>>>>(4096);

    //let pool1 = Arc::clone(&pool);
    std::thread::spawn(|| recv_pkt(socket, tx_payload));

    let lo_freq=args.lo_megahz*1e6;
    let max_freq=(RAW_SAMP_RATE/2)  as f64;
    let lo_freq_ch=(lo_freq/max_freq*(N_PT_PER_FRAME/2) as f64) as usize;

    std::thread::spawn(move || ddc_x8(rx_payload, tx_ddc, lo_freq_ch));
    let mut last_print_time = Instant::now();
    let mut last_printed_second = 0; // 记录上次打印的秒数
    let t0=Local::now();
    let mut nsamples=0;
    for i in 0.. {
        let ddc_data = rx_ddc.recv().unwrap();
        nsamples+=ddc_data.len();
        if let Some(ref outname) = args.outname {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(outname)
                .unwrap();
            f.write_all(slice_as_u8(&ddc_data)).unwrap();
        }
        if last_print_time.elapsed() >= Duration::from_secs(1) {
            let current_time = Local::now();
            let dt=current_time-t0;
            
            let current_second = current_time.timestamp(); // 获取当前秒数

            // 仅当秒数变化时才打印（避免 1s 内多次打印）
            if current_second != last_printed_second {
                println!("{} {} {} {} {} MSps", current_time.format("%Y-%m-%d %H:%M:%S"), i, rx_ddc.len(), dt.num_seconds(), nsamples as f64/1e6/dt.num_seconds() as f64);
                last_printed_second = current_second;
            }

            last_print_time = Instant::now(); // 重置计时
        }
    }
}
