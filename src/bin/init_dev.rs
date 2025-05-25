use std::{fs::File, io::Write};

use clap::Parser;
use num::Complex;

use sdaa_data::{sdr::{Sdr, SdrSmpRate}, utils::slice_as_u8};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'A', value_name = "remote ctrl ip:port")]
    remote_ctrl_addr: String,

    #[clap(
        short = 'L',
        value_name = "local ctrl ip:port",
        default_value = "0.0.0.0:3001"
    )]
    local_ctrl_addr: String,
}

fn main() {
    //let (tx,rx)=bounded(256);

    use sdaa_data::sdr::SdrCtrl;

    let args = Args::parse();

    let sdr_ctrl=SdrCtrl{remote_ctrl_addr: args.remote_ctrl_addr.parse().unwrap(),
        local_ctrl_addr: args.local_ctrl_addr.parse().unwrap()};


    //std::thread::sleep(std::time::Duration::from_secs(2));
    sdr_ctrl.stream_stop();
    sdr_ctrl.wait_until_locked(60);
    let summary=sdr_ctrl.init();
    assert_eq!(summary.normal_reply.len(),1);
}

