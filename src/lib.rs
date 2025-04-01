#![feature(portable_simd)]

pub mod ddc;
pub mod fir;
pub mod payload;
pub mod pipeline;
pub mod utils;
pub mod bindings;


pub type RawType = i16;
pub type Ftype = f32;
pub const RAW_SAMP_RATE:usize=480_000_000;
