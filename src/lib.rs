#![feature(portable_simd)]

pub mod ddc;
pub mod fir;
pub mod payload;
pub mod pipeline;
pub mod utils;

pub type RawType = i16;
pub type Ftype = f32;
