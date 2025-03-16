pub const N_PT_PER_FRAME: usize = 4096;

#[repr(C)]
pub struct Payload {
    pub header: u32,
    pub version: u32,
    pub pkt_cnt: u64,
    pub base_id: i64,
    pub port_id: i64,
    pub npt_per_frame: u64,
    pub _reserved: u64,
    pub data: [i16; N_PT_PER_FRAME],
}

impl Default for Payload {
    fn default() -> Self {
        Self {
            header: 0x12345678,
            version: 0,
            pkt_cnt: 0,
            base_id: 0,
            port_id: 0,
            npt_per_frame: N_PT_PER_FRAME as u64,
            _reserved: 0,
            data: [0; N_PT_PER_FRAME],
        }
    }
}

impl Payload{
    pub fn copy_header(&mut self, rhs: &Self){
        self.header=rhs.header;
        self.version=rhs.version;
        self.pkt_cnt=rhs.pkt_cnt;
        self.base_id=rhs.base_id;
        self.port_id=rhs.port_id;
        self.npt_per_frame=rhs.npt_per_frame;
    }
}
