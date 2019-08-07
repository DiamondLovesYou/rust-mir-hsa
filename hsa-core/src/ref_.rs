
use std::cmp;
use std::fmt;
use std::marker::{PhantomData, };
use std::mem::transmute;
use std::ops::*;

use crate::ptr::NonNull;
use ptr::{AccelPtr, PtrTy, PtrRefTy};
use platform::is_host;

/// A type which when dereferenced will automatically use the correct pointer.
#[derive(Clone, Copy)]
pub struct Ref<'a, T>
  where T: ?Sized,
{
  pub(crate) _owner: PhantomData<&'a T>,
  pub(crate) ptr: NonNull<T>,
}

impl<'a, T> Ref<'a, T>
  where T: ?Sized,
{
}

impl<'a, T> AsRef<T> for Ref<'a, T> {
  fn as_ref(&self) -> &T {
    unsafe { self.ptr.as_local_ref() }
  }
}
impl<'a, T> Deref for Ref<'a, T> {
  type Target = T;
  fn deref(&self) -> &T {
    unsafe { self.ptr.as_local_ref() }
  }
}
impl<'a, T> fmt::Debug for Ref<'a, T>
  where T: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self.as_ref())
  }
}

/// A type which when dereferenced will automatically use the correct pointer.
pub struct Mut<'a, T>
  where T: ?Sized,
{
  pub(crate) _owner: PhantomData<&'a mut T>,
  pub(crate) ptr: NonNull<T>,
}

impl<'a, T> Mut<'a, T>
  where T: ?Sized,
{
  /// Create a temporary (in that self can be unborrowed, and used again)
  /// borrow
  pub fn into_ref<'b>(&'b self) -> Ref<'b, T>
    where 'a: 'b,
  {
    Ref {
      _owner: PhantomData,
      ptr: self.ptr.clone(),
    }
  }
}

impl<'a, T> AsRef<T> for Mut<'a, T> {
  fn as_ref(&self) -> &T {
    unsafe { self.ptr.as_local_ref() }
  }
}
impl<'a, T> AsMut<T> for Mut<'a, T> {
  fn as_mut(&mut self) -> &mut T {
    unsafe { self.ptr.as_local_mut() }
  }
}
impl<'a, T> Deref for Mut<'a, T> {
  type Target = T;
  fn deref(&self) -> &T {
    unsafe { self.ptr.as_local_ref() }
  }
}
impl<'a, T> DerefMut for Mut<'a, T> {
  fn deref_mut(&mut self) -> &mut T {
    unsafe { self.ptr.as_local_mut() }
  }
}
impl<'a, T> fmt::Debug for Mut<'a, T>
  where T: fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self.as_ref())
  }
}

/// A newtype which asserts on deref if the host attemps to read from the
/// wrapped inner data. This is intended to be used behind a reference;
/// ie via `&AccelRefRaw2<T>` or `&mut AccelRefRaw2<T>`
///
/// This type should never be constructed. It should always be "created"
/// by transmuting a `&T` into `&AccelRefRaw2<T>`.
///
/// Such references are often only dereferencable
/// on the specific accelerator it is associated with.
/// XXX this still marginally unsafe: we don't check that the pointer
/// is actually accessible by the running processor, ie the pointer could
/// be accessible from a different accelerator, which if `T` is Sized can
/// still be dereferenced
/// TODO add intrinsic to get the running accelerator id.
#[repr(transparent)]
pub struct AccelRefRaw<T>(T)
  where T: ?Sized;

impl<T> AccelRefRaw<T>
  where T: ?Sized,
{
  pub unsafe fn ref_from_ptr<'a, P>(ptr: AccelPtr<P>) -> Option<&'a Self>
    where P: PtrTy<ElementTy = T> + PtrRefTy<'a, RefTy = &'a T> + 'a,
  {
    ptr.as_ref_ty().map(|r| transmute(r) )
  }
  pub unsafe fn mut_from_ptr<'a, P>(ptr: AccelPtr<P>) -> Option<&'a mut Self>
    where P: PtrTy<ElementTy = T> + PtrRefTy<'a, RefTy = &'a mut T> + 'a,
  {
    ptr.as_ref_ty().map(|r| transmute(r) )
  }

  pub fn as_ref(&self) -> &T {
    assert!(!is_host());
    &self.0
  }
  pub fn as_mut(&mut self) -> &mut T {
    assert!(!is_host());
    &mut self.0
  }

  pub unsafe fn unchecked_as_ref(&self) -> &T { &self.0 }
  pub unsafe fn unchecked_as_mut(&mut self) -> &mut T { &mut self.0 }
}
impl<T> AccelRefRaw<[T]>
  where T: Sized,
{
  /// This is always safe to call, host or device, due to the way
  /// DSTs/slices work. They are fat pointers (ie `&self` is twice
  /// the size of a pointer), so reading the length just reads the
  /// second pointer sized integer (a `usize`) from `&self`.
  pub fn len(&self) -> usize {
    unsafe {
      self.unchecked_as_ref().len()
    }
  }

  // TODO implement the reset of the [T] subslice methods (the ones which
  // do not read any element in the slice).
}

// XXX The copy is generated by the compiler, so we can't stop the
// reads if on host.
// This will probably never be fixable.
/*
impl<T> Copy for AccelRefRaw2<T>
  where T: Copy,
{ }
*/
impl<T> Clone for AccelRefRaw<T>
  where T: Clone,
{
  fn clone(&self) -> Self {
    AccelRefRaw(self.as_ref().clone())
  }
}
impl<T> Eq for AccelRefRaw<T>
  where T: Eq,
{ }
impl<T, U> PartialEq<AccelRefRaw<U>> for AccelRefRaw<T>
  where T: PartialEq<U>,
{
  fn eq(&self, rhs: &AccelRefRaw<U>) -> bool {
    self.as_ref().eq(rhs.as_ref())
  }
}
impl<T> Ord for AccelRefRaw<T>
  where T: Ord,
{
  fn cmp(&self, rhs: &Self) -> cmp::Ordering {
    self.as_ref().cmp(rhs.as_ref())
  }
}
impl<T, U> PartialOrd<AccelRefRaw<U>> for AccelRefRaw<T>
  where T: PartialOrd<U>,
{
  fn partial_cmp(&self, rhs: &AccelRefRaw<U>) -> Option<cmp::Ordering> {
    self.as_ref().partial_cmp(rhs.as_ref())
  }
}

impl<T> fmt::Debug for AccelRefRaw<T>
  where T: ?Sized + fmt::Debug,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if is_host() {
      fmt::Pointer::fmt(&self, f)
    } else {
      fmt::Debug::fmt(&self.0, f)
    }
  }
}
/// XXX this still marginally unsafe: we don't check that the pointer
/// is actually accessible by the running processor, ie the pointer could
/// be accessible from a different accelerator.
/// TODO add intrinsic to get the running accelerator id.
impl<T> Deref for AccelRefRaw<T>
  where T: ?Sized,
{
  type Target = T;
  fn deref(&self) -> &T {
    self.as_ref()
  }
}
impl<T> DerefMut for AccelRefRaw<T>
  where T: ?Sized,
{
  fn deref_mut(&mut self) -> &mut T {
    self.as_mut()
  }
}
impl<T, U> CoerceUnsized<AccelRefRaw<U>> for AccelRefRaw<T>
  where
    T: CoerceUnsized<U> + ?Sized,
    U: ?Sized,
{ }