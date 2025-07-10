use lockfree_object_pool::LinearOwnedReusable;
use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
};

use clap::Parser;
use crossbeam::channel::bounded;
use sdaa_data::{
    RAW_SAMP_RATE,
    payload::Payload,
    pipeline::{MaybeMulticastReceiver, RecvCmd, pkt_wf, recv_pkt},
    utils::slice_as_u8,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'a', long = "addr", value_name = "ip:port")]
    local_addr: String,

    #[clap(short = 'm', long = "maddr", value_name = "ip")]
    multicast_addr: Option<String>,

    #[clap(short = 'o', long = "out", value_name = "out name")]
    outname: Option<String>,

    #[clap(short = 'c', long = "nch", value_name = "num of ch")]
    nch: usize,

    #[clap(
        short = 'b',
        long = "nbatch",
        value_name = "fft batch",
        default_value_t = 0
    )]
    nbatch: usize,

    #[clap(
        short = 'n',
        long = "nint",
        value_name = "num of fft per integration",
        default_value_t = 1024
    )]
    nint: usize,
}

fn main() {
    //let (tx,rx)=bounded(256);
    let args = Args::parse();
    let nint = args.nint;
    let nbatch = if args.nbatch == 0 { nint } else { args.nbatch };
    let addr = args.local_addr.parse::<SocketAddrV4>().unwrap();

    let socket = if let Some(mcast_addr_str) = args.multicast_addr {
        let local_iface = addr.ip().clone(); // 改成你网卡的实际 IPv4 地址
        let mcast_addr = mcast_addr_str.parse::<Ipv4Addr>().unwrap();
        assert!(mcast_addr.is_multicast());

        MaybeMulticastReceiver::new(
            SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), addr.port()),
            Some((mcast_addr, local_iface)),
        )
        .unwrap()
    } else {
        UdpSocket::bind(&addr).unwrap().into()
    };

    let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(16384);
    let (tx_wf, rx_wf) = bounded::<LinearOwnedReusable<Vec<f32>>>(4096);
    let (tx_recv_cmd, rx_recv_cmd) = bounded(1024);

    ctrlc::set_handler(move || {
        println!("Caught Ctrl+C");
        tx_recv_cmd.send(RecvCmd::Destroy).unwrap();
    })
    .expect("Error setting Ctrl+C handler");

    //let pool1 = Arc::clone(&pool);
    std::thread::spawn(move || pkt_wf(rx_payload, tx_wf, args.nch, nbatch, nint));
    //std::thread::sleep(std::time::Duration::from_secs(1));
    std::thread::spawn(|| recv_pkt(socket, tx_payload, rx_recv_cmd));
    let dt = (args.nch * 2 * args.nint) as f64 / RAW_SAMP_RATE as f64;

    //let mut dump_file = None;
    //let mut outfile = args.outname.map(|outname| File::create(&outname).unwrap());
    let mut time_elapsed = 0.0;
    let mut old_time_elapsed_integer = 0;
    let dt_per_iter = dt * (nbatch as f64 / nint as f64);

    println!(
        "dt={dt}={}/{} dt per iter={}",
        args.nch * 2 * args.nint,
        RAW_SAMP_RATE,
        dt_per_iter
    );
    for _i in 0.. {
        if let Ok(x) = rx_wf.recv() {
            time_elapsed += dt_per_iter;
            if time_elapsed as usize != old_time_elapsed_integer {
                println!("{time_elapsed}");
                old_time_elapsed_integer = time_elapsed as usize;
            }

            args.outname
                .as_ref()
                .map(|outname| {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(outname)
                        .unwrap()
                })
                .iter_mut()
                .for_each(|f| {
                    f.write_all(slice_as_u8(&x[..])).unwrap();
                });
        }else{
            break;
        }
        //let x = rx_wf.recv().unwrap();
    }
}
