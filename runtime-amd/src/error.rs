
use std::error::Error as StdError;
use std::fmt;
use std::geobacter::kernel::KernelInstanceRef;
use std::io::Error as IoError;

use alloc_wg::alloc::Layout;

use hsa_rt::queue::QueueError;

use crate::HsaError;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
  Generic(Box<dyn StdError + Send + Sync + 'static>),
  LoadRustcMetadata(Box<dyn StdError + Send + Sync + 'static>),
  Hsa(HsaError),
  AmdComgr(amd_comgr::error::Error),
  Cmd(Box<dyn StdError + Send + Sync + 'static>),
  Io(IoError),
  KernelInfoElf(goblin::error::Error),
  KernelInfoMessagePack(rmps::decode::Error),
  ConvertKernelInstance(KernelInstanceRef<'static>),
  ContextDead,
  Codegen,
  Linking,
  NoCpuAgent,
  NoGpuAgent,
  NoGpuAgentIsa,
  UnknownAmdGpuArch(String),
  UnsupportedBigEndianHost,
  MissingKernelArgumentsRegion,
  MissingHostLocalFineGrainedPool,
  MissingHostLocalCoarseGrainedPool,
  MissingKernelSymbol(String),
  UnexpectedNullKernelObject,
  MissingKernelMetadataNote,
  CodegenInitRoot(Box<Error>),
  CodegenInitConditions(Box<Error>),
  CodegenPreCodegen(Box<Error>),
  CodegenPostCodegen(Box<Error>),
  Underflow,
  Overflow,
  /// The dispatch grid has a zero length along one or more of it's axes.
  ZeroGridLaunchAxis,
  KernelArgsPoolOom,
  HsaQueue(QueueError),
  Alloc(Layout),
  KernelWorkgroupDimTooLargeForDevice,
  KernelWorkgroupLenTooLargeForDevice,
  LaunchGridDimTooLargeForDevice,
  LaunchGridLenTooLargeForDevice,
}
impl StdError for Error {
  fn source(&self) -> Option<&(dyn StdError + 'static)> {
    match self {
      Error::Generic(ref inner) |
      Error::Cmd(ref inner) => Some(&**inner),
      Error::Hsa(inner) => Some(inner),
      Error::Io(inner) => Some(inner),
      Error::KernelInfoElf(inner) => Some(inner),
      Error::CodegenInitConditions(inner) |
      Error::CodegenInitRoot(inner) |
      Error::CodegenPostCodegen(inner) |
      Error::CodegenPreCodegen(inner) => Some(inner),
      _ => None,
    }
  }
}
impl From<HsaError> for Error {
  #[inline(always)]
  fn from(v: HsaError) -> Self {
    match v {
      HsaError::Overflow => Error::Overflow,
      _ => Error::Hsa(v),
    }
  }
}
impl From<amd_comgr::error::Error> for Error {
  #[inline(always)]
  fn from(v: amd_comgr::error::Error) -> Self {
    Error::AmdComgr(v)
  }
}
impl From<IoError> for Error {
  #[inline(always)]
  fn from(v: IoError) -> Self {
    Error::Io(v)
  }
}
impl From<goblin::error::Error> for Error {
  #[inline(always)]
  fn from(v: goblin::error::Error) -> Error {
    Error::KernelInfoElf(v)
  }
}
impl From<rmps::decode::Error> for Error {
  #[inline(always)]
  fn from(v: rmps::decode::Error) -> Error {
    Error::KernelInfoMessagePack(v)
  }
}
impl From<QueueError> for Error {
  #[inline(always)]
  fn from(v: QueueError) -> Error {
    Error::HsaQueue(v)
  }
}
impl From<alloc_wg::collections::TryReserveError> for Error {
  #[inline(always)]
  fn from(v: alloc_wg::collections::TryReserveError) -> Error {
    match v {
      alloc_wg::collections::TryReserveError::CapacityOverflow => Error::Overflow,
      alloc_wg::collections::TryReserveError::AllocError {
        layout, ..
      } => Error::Alloc(layout),
    }
  }
}
impl From<grt_core::codegen::error::Error<Error>> for Error {
  #[inline(always)]
  fn from(v: grt_core::codegen::error::Error<Error>) -> Self {
    use grt_core::codegen::error::Error::*;

    match v {
      Io(_, err) => Error::Io(err),
      LoadMetadata(err) => Error::LoadRustcMetadata(err),
      ConvertKernelInstance(ki) => Error::ConvertKernelInstance(ki),
      Codegen => Error::Codegen,
      Linking => Error::Linking,
      InitRoot(inner) => Error::CodegenInitRoot(Box::new(inner)),
      InitConditions(inner) => Error::CodegenInitConditions(Box::new(inner)),
      PreCodegen(inner) => Error::CodegenPreCodegen(Box::new(inner)),
      PostCodegen(inner) => Error::CodegenPostCodegen(Box::new(inner)),
      ContextDead => Error::ContextDead,
    }
  }
}
impl From<Box<dyn StdError + Send + Sync + 'static>> for Error {
  fn from(v: Box<dyn StdError + Send + Sync + 'static>) -> Self {
    v.downcast()
      .map(|v| *v )
      .or_else(|v| {
        v.downcast()
          .map(|v: Box<IoError>| Error::Io(*v) )
      })
      .unwrap_or_else(Error::Generic)
  }
}
impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
