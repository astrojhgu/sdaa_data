use clap::Parser;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short = 'r', value_name = "iq rate 240 or 120", default_value_t=240)]
    iq_rate: usize,
}

#[cfg(feature = "cuda")]
fn main() {
    //let (tx,rx)=bounded(256);
    use crossbeam::channel::bounded;
    use sdaa_data::{ddc::N_PT_PER_FRAME, pipeline::{fake_dev, pkt_ddc, DdcCmd}};

    let args = Args::parse();
    let (tx_payload, rx_payload)=bounded(1024);
    let (tx_recv_cmd, rx_recv_cmd)=bounded(32);
    let (tx_ddc, rx_ddc)=bounded(1024);
    let (tx_ddc_cmd, rx_ddc_cmd)=bounded(1024);

    tx_ddc_cmd
        .send(DdcCmd::LoCh(N_PT_PER_FRAME as isize/4))
        .expect("failed to send loch");

    std::thread::spawn(move || fake_dev(tx_payload, rx_recv_cmd));
    std::thread::spawn(move || {
        let ndec=480/args.iq_rate;
        let fir_coeffs = match args.iq_rate {
            240 => sdaa_data::ddc::fir_coeffs_full(),
            120 => sdaa_data::ddc::fir_coeffs_half(),
            _ => panic!("invalid iq rate"),
        };
        pkt_ddc(rx_payload, tx_ddc, ndec, rx_ddc_cmd, tx_recv_cmd, &fir_coeffs)});

    for _i in 0.. {
        let _ddc = rx_ddc.recv().expect("failed to recv ddc payload");
    }
}

