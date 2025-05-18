use crate::ddc::N_PT_PER_FRAME;

pub struct WfResource{
    pub res:*mut crate::bindings::cuwf::Resource,
    pub nch: usize, 
    pub nbatch: usize, 
    pub nint: usize,
}

impl Drop for WfResource {
    fn drop(&mut self) {
        unsafe {
            crate::bindings::cuwf::destroy_resource(self.res);
        }
    }
}

impl WfResource {
    pub fn new(nch: usize, nbatch: usize, nint: usize) -> Self {
        let res = unsafe { crate::bindings::cuwf::init_resource(nch as i32, N_PT_PER_FRAME as i32, nbatch as i32, nint as i32) };
        if res.is_null() {
            panic!("Failed to create resource");
        }
        Self { res, nch, nbatch, nint }
    }

    pub fn process(&mut self, input: &[i16], output: &mut [f32])->bool {
        assert_eq!(output.len(), self.nch * self.nbatch/self.nint);
        unsafe {
            crate::bindings::cuwf::waterfall(self.res, input.as_ptr(), input.len(), output.as_mut_ptr())
        }
    }
}
