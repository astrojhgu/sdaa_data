#![allow(static_mut_refs)]

use std::net::{Ipv4Addr, SocketAddrV4};

use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::LinearOwnedReusable;
use num::Complex;

use crate::{
    ddc::{M, N_PT_PER_FRAME}, payload::Payload, pipeline::{DdcCmd, RecvCmd}, sdr::{Sdr, RawSdr}
};

pub const NDEC: usize = 4;

pub struct CSdr {
    sdr_dev: Sdr,
    rx_iq: Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>,
    tx_cmd: Sender<DdcCmd>,
    buffer: Option<LinearOwnedReusable<Vec<Complex<f32>>>>,
    cursor: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CComplex {
    pub re: f32,
    pub im: f32,
}

#[no_mangle]
pub extern "C" fn new_sdr_device(
    remote_ctrl_ip: u32,
    local_ctrl_port: u16,
    local_payload_ip: u32,
    local_payload_port: u16,
) -> *mut CSdr {
    let remote_ctrl_addr = SocketAddrV4::new(Ipv4Addr::from(remote_ctrl_ip), 3000);
    let local_ctrl_addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), local_ctrl_port);
    let local_payload_addr =
        SocketAddrV4::new(Ipv4Addr::from(local_payload_ip), local_payload_port);

    let (sdr_dev, rx_iq, tx_cmd) = Sdr::new(remote_ctrl_addr, local_ctrl_addr, local_payload_addr);

    if let Some(x)=sdr_dev.ctrl.awaken_and_locked(){
        if !x{
            //panic!();
            sdr_dev.ctrl.wakeup();
            sdr_dev.ctrl.wait_until_locked(60);    
        }
    }else{
        panic!("abnormal reply")
    }
    sdr_dev.ctrl.init();
    sdr_dev.ctrl.sync();

    Box::into_raw(Box::new(CSdr {
        sdr_dev,
        rx_iq,
        tx_cmd,
        buffer: None,
        cursor: 0,
    }))
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn free_sdr_device(csdr: *mut CSdr) {
    if !csdr.is_null() {
        let obj = unsafe { Box::from_raw(csdr) };
        let CSdr {
            sdr_dev: _,
            rx_iq,
            tx_cmd,
            buffer: _,
            cursor: _,
        } = *obj;
        tx_cmd.send(DdcCmd::Destroy).unwrap();
        drop(tx_cmd);
        drop(rx_iq);
    }
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn set_lo_ch(csdr: *mut CSdr, lo_ch: i32) {
    if csdr.is_null() {
        return;
    }

    let obj = unsafe { &mut *csdr };
    obj.tx_cmd.send(DdcCmd::LoCh(lo_ch as isize)).unwrap();
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn fetch_data(csdr: *mut CSdr, buf: *mut CComplex, npt: usize) {
    if csdr.is_null() {
        return;
    }

    let obj = unsafe { &mut *csdr };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf as *mut Complex<f32>, npt) };
    if obj.buffer.is_none() {
        obj.buffer = Some(obj.rx_iq.recv().unwrap());
        obj.cursor = 0;
    }

    let mut written = 0;
    let total = npt;
    while written < total {
        let available = obj.buffer.as_ref().unwrap().len() - obj.cursor;
        if available == 0 {
            obj.buffer = Some(obj.rx_iq.recv().unwrap());
            obj.cursor = 0;
            continue;
        }
        let copy_len = (total - written).min(available);
        buf[written..written + copy_len]
            .copy_from_slice(&obj.buffer.as_ref().unwrap()[obj.cursor..obj.cursor + copy_len]);
        obj.cursor += copy_len;
        written += copy_len;
    }
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub extern "C" fn get_mtu() -> usize {
    N_PT_PER_FRAME * M / NDEC
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn start_data_stream(csdr: *mut CSdr) {
    let obj = unsafe { &mut *csdr };
    obj.sdr_dev.ctrl.stream_start();
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn stop_data_stream(csdr: *mut CSdr) {
    let obj = unsafe { &mut *csdr };
    obj.sdr_dev.ctrl.stream_stop();
}

pub struct CRawSdr {
    sdr_dev: RawSdr,
    rx_payload: Receiver<LinearOwnedReusable<Payload>>,
    tx_cmd: Sender<RecvCmd>,
    buffer: Option<LinearOwnedReusable<Payload>>,
    cursor: usize,
}


#[no_mangle]
pub extern "C" fn new_raw_sdr_device(
    remote_ctrl_ip: u32,
    local_ctrl_port: u16,
    local_payload_ip: u32,
    local_payload_port: u16,
) -> *mut CRawSdr {
    let remote_ctrl_addr = SocketAddrV4::new(Ipv4Addr::from(remote_ctrl_ip), 3000);
    let local_ctrl_addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), local_ctrl_port);
    let local_payload_addr =
        SocketAddrV4::new(Ipv4Addr::from(local_payload_ip), local_payload_port);

    let (sdr_dev, rx_payload, tx_cmd) = RawSdr::new(remote_ctrl_addr, local_ctrl_addr, local_payload_addr);

    if let Some(x)=sdr_dev.ctrl.awaken_and_locked(){
        if !x{
            //panic!();
            sdr_dev.ctrl.wakeup();
            sdr_dev.ctrl.wait_until_locked(60);    
        }
    }else{
        panic!("abnormal reply")
    }
    sdr_dev.ctrl.init();
    sdr_dev.ctrl.sync();

    Box::into_raw(Box::new(CRawSdr {
        sdr_dev,
        rx_payload,
        tx_cmd,
        buffer: None,
        cursor: 0,
    }))
}


/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn free_raw_sdr_device(csdr: *mut CRawSdr) {
    if !csdr.is_null() {
        let obj = unsafe { Box::from_raw(csdr) };
        let CRawSdr {
            sdr_dev: _,
            rx_payload,
            tx_cmd,
            buffer: _,
            cursor: _,
        } = *obj;
        tx_cmd.send(RecvCmd::Destroy).unwrap();
        drop(tx_cmd);
        drop(rx_payload);
    }
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn fetch_raw_data(csdr: *mut CRawSdr, buf: *mut i16, npt: usize) {
    if csdr.is_null() {
        return;
    }

    let obj = unsafe { &mut *csdr };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf as *mut i16, npt) };
    if obj.buffer.is_none() {
        obj.buffer = Some(obj.rx_payload.recv().unwrap());
        obj.cursor = 0;
    }

    let mut written = 0;
    let total = npt;
    while written < total {
        let available = N_PT_PER_FRAME - obj.cursor;
        if available == 0 {
            obj.buffer = Some(obj.rx_payload.recv().unwrap());
            obj.cursor = 0;
            continue;
        }
        let copy_len = (total - written).min(available);
        buf[written..written + copy_len]
            .copy_from_slice(&obj.buffer.as_ref().unwrap().data[obj.cursor..obj.cursor + copy_len]);
        obj.cursor += copy_len;
        written += copy_len;
    }
}

#[no_mangle]
pub unsafe extern "C" fn start_raw_data_stream(csdr: *mut CRawSdr) {
    let obj = unsafe { &mut *csdr };
    obj.sdr_dev.ctrl.stream_start();
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn stop_raw_data_stream(csdr: *mut CRawSdr) {
    let obj = unsafe { &mut *csdr };
    obj.sdr_dev.ctrl.stream_stop();
}
