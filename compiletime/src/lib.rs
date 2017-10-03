#![feature(intrinsics)]
#![feature(unboxed_closures)]
#![feature(plugin)]

#![plugin(hsa_rustc_plugin)]

mod intrinsics {
  extern "rust-intrinsic" {
    pub fn json_kernel_info_for<F, Args, Ret>(f: &F) -> &'static str
      where F: Fn<Args, Output=Ret>;

    // Returns global static crate kernels structure. This structure
    // is stored in every crate, under an implementation defined name.
    //pub fn crate_kernels() -> &'static StaticCrateKernels;
  }
}

pub fn json_kernel_info_for<F, Args, Ret>(f: &F) -> &'static str
  where F: Fn<Args, Output=Ret>
{
  unsafe {
    intrinsics::json_kernel_info_for(f)
  }
}

