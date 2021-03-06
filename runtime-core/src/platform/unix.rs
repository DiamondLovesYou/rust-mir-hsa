
use std::env::{split_paths, var_os};
use std::ffi::{c_void, CStr, OsString};
use std::io::{Error as IoError, };
use std::os::raw::{c_int, c_char};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{PathBuf};
use std::slice::from_raw_parts;
use std::str::from_utf8_unchecked;

use goblin::elf::dynamic::*;

use libc::*;

pub fn self_exe_path() -> Result<PathBuf, IoError> {
  use std::fs::read_link;

  const P: &'static str = "/proc/self/exe";

  Ok(read_link(P)?)
}

fn runpaths() -> Vec<PathBuf> {

  #[cfg(target_pointer_width = "64")]
  union ElfDUn {
    d_val: Elf64_Xword,
    d_ptr: Elf64_Addr,
  }
  #[cfg(target_pointer_width = "64")]
  struct ElfDyn {
    d_tag: Elf64_Sxword,
    d_un: ElfDUn,
  }
  #[cfg(target_pointer_width = "32")]
  union ElfDUn {
    d_val: Elf64_Word,
    d_ptr: Elf64_Addr,
  }
  #[cfg(target_pointer_width = "32")]
  struct ElfDyn {
    d_tag: Elf32_Sword,
    d_un: ElfDUn,
  }

  #[derive(Default)]
  struct State {
    paths: Vec<PathBuf>,
  }
  impl State {
    fn with_phdr(&mut self, info: &mut dl_phdr_info) {
      let phdrs = unsafe {
        from_raw_parts(info.dlpi_phdr, info.dlpi_phnum as _)
      };
      let section = match phdrs.iter().find(|phdr| phdr.p_type == PT_DYNAMIC ) {
        Some(s) => s,
        None => { return; },
      };

      let mut ptr = (info.dlpi_addr + section.p_vaddr) as *const ElfDyn;
      let strtbl_ptr;
      // Find the string table:
      loop {
        let entry = unsafe { &*ptr };
        if entry.d_tag == DT_NULL as i64 {
          // String table not found?! Not much more we can do here.
          return;
        }

        if entry.d_tag == DT_STRTAB as i64 {
          strtbl_ptr = unsafe {
            entry.d_un.d_ptr as *const u8
          };
          break;
        }

        ptr = unsafe { ptr.add(1) };
      }

      ptr = (info.dlpi_addr + section.p_vaddr) as *const ElfDyn;
      // Now find the DT_RUNPATH entry
      let runpath_ptr;
      loop {
        let entry = unsafe { &*ptr };
        if entry.d_tag == DT_NULL as i64 {
          // DT_RUNPATH not found?! Not much more we can do here.
          return;
        }

        if entry.d_tag == DT_RUNPATH as i64 {
          runpath_ptr = unsafe {
            strtbl_ptr.add(entry.d_un.d_val as _)
          };
          break;
        }

        ptr = unsafe { ptr.add(1) };
      }

      let runpaths_cstr = unsafe {
        CStr::from_ptr(runpath_ptr as *const c_char)
      };

      let runpaths = match runpaths_cstr.to_str() {
        Ok(s) => s,
        Err(e) => {
          let s = runpaths_cstr.to_bytes();
          unsafe {
            from_utf8_unchecked(&s[..e.valid_up_to()])
          }
        }
      };
      self.paths.extend(split_paths(runpaths));
    }
  }

  unsafe extern "C" fn callback(info: *mut dl_phdr_info, _size: usize,
                                datap: *mut c_void) -> c_int
  {
    let data = &mut *(datap as *mut State);
    data.with_phdr(&mut *info);

    1 // we only care about /proc/self/exe
  }

  let mut state = State::default();
  unsafe {
    dl_iterate_phdr(Some(callback), &mut state as *mut State as *mut _);
  }

  state.paths
}

fn expand_dynamic_tokens(input: impl Iterator<Item = PathBuf>)
  -> impl Iterator<Item = PathBuf>
{
  #![cfg_attr(test, allow(dead_code))]

  let mut self_exe_dir = None;
  fn get_origin<'a>(self_exe_dir: &'a mut Option<Box<[u8]>>) -> &'a [u8] {
    if let Some(dir) = self_exe_dir {
      return &dir[..];
    }

    let p = self_exe_path()
      .expect("/proc/self/exe")
      .parent()
      .unwrap()
      .as_os_str()
      .as_bytes()
      .to_vec()
      .into_boxed_slice();
    *self_exe_dir = Some(p);
    return self_exe_dir.as_ref().unwrap();
  }

  #[cfg(target_pointer_width = "32")]
  const LIB: &'static [u8] = b"lib";
  #[cfg(target_pointer_width = "64")]
  const LIB: &'static [u8] = b"lib64";

  let mut platform = None;
  fn get_platform<'a>(platform: &'a mut Option<&'static [u8]>) -> &'a [u8] {
    unsafe {
      if let Some(platform) = platform {
        return platform;
      }

      let p = getauxval(AT_PLATFORM) as usize as *const c_char;
      let b = CStr::from_ptr(p).to_bytes();
      *platform = Some(b);
      return b;
    }
  }

  const ORIGIN_V: &'static [u8] = b"ORIGIN";
  const LIB_V: &'static [u8] = b"LIB";
  const PLATFORM_V: &'static [u8] = b"PLATFORM";

  #[derive(Clone, Copy, Debug)]
  enum Var {
    Origin,
    Lib,
    Platform,
  }

  input
    .map(move |path| {
      if !path.as_os_str().as_bytes().contains(&('$' as u8)) { return path; }

      let mut out = Vec::new();
      let mut splitter = path.as_os_str().as_bytes().split(|&b| b == '$' as u8 );
      if let Some(first) = splitter.next() {
        out.extend_from_slice(first);
      }
      for mut split in splitter {
        if split.is_empty() {
          break;
        }

        let all = split;
        let expect_bracket = split[0] == '{' as u8;
        if expect_bracket {
          split = &split[1..];
        }

        const EMPTY: &'static [u8] = &[];

        let (var, rest) = if expect_bracket {
          split.iter().position(|&b| b == '}' as u8 )
            .map(|i| {
              if i + 1 >= split.len() {
                (&split[..i], EMPTY)
              } else {
                (&split[..i], &split[i + 1..])
              }
            })
            .unwrap_or((&split[..], EMPTY))
        } else {
          split.iter()
            .position(|&b| !((b as char).is_alphanumeric() || b == '_' as u8) )
            .map(|i| {
              split.split_at(i)
            })
            .unwrap_or((&split[..], EMPTY))
        };

        let var = if var == ORIGIN_V {
          Var::Origin
        } else if var == LIB_V {
          Var::Lib
        } else if var == PLATFORM_V {
          Var::Platform
        } else {
          out.push('$' as u8);
          out.extend_from_slice(all);
          continue;
        };

        let expanded = match var {
          Var::Origin => get_origin(&mut self_exe_dir),
          Var::Lib => LIB,
          Var::Platform => get_platform(&mut platform),
        };
        out.extend_from_slice(expanded);
        out.extend_from_slice(rest);
      }

      PathBuf::from(OsString::from_vec(out))
    })
}

pub fn dylib_search_paths() -> Vec<PathBuf> {
  let paths = var_os("LD_LIBRARY_PATH").unwrap_or("".into());
  let paths = split_paths(&paths).chain(runpaths().into_iter());
  expand_dynamic_tokens(paths)
    .collect()
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn origin() {
    let paths = vec![
      PathBuf::from("$ORIGIN"),
      PathBuf::from("${ORIGIN}"),
    ];
    let expanded = expand_dynamic_tokens(paths.into_iter())
      .collect::<Vec<_>>();
    for expanded in expanded.iter() {
      assert!(expanded.exists() && expanded.is_dir());
    }
  }

  #[test]
  fn not_origin() {
    let paths = vec![
      PathBuf::from("$ORIGIN_NOT"),
      PathBuf::from("${ORIGIN_NOT}"),
    ];
    let expanded = expand_dynamic_tokens(paths.clone().into_iter())
      .collect::<Vec<_>>();
    for (expanded, original) in expanded.iter().zip(paths.iter()) {
      assert_eq!(expanded, original);
    }
  }

  #[test]
  fn unknown_var() {
    let paths = vec![
      PathBuf::from("$UNKNOWN_VAR0123456789/../lib"),
      PathBuf::from("${UNKNOWN_VAR0123456789}/../lib"),
    ];
    let expanded = expand_dynamic_tokens(paths.clone().into_iter())
      .collect::<Vec<_>>();
    for (expanded, original) in expanded.iter().zip(paths.iter()) {
      assert_eq!(expanded, original);
    }
  }

  #[test]
  fn lib() {
    let paths = vec![
      PathBuf::from("$LIB"),
      PathBuf::from("${LIB}"),
    ];
    let expanded = expand_dynamic_tokens(paths.into_iter())
      .collect::<Vec<_>>();
    for expanded in expanded.iter() {
      let expanded = expanded.to_str()
        .expect("should always be utf8");
      assert!(!expanded.contains("LIB"));
    }
  }

  #[test]
  fn platform() {
    let paths = vec![
      PathBuf::from("$PLATFORM"),
      PathBuf::from("${PLATFORM}"),
    ];
    let expanded = expand_dynamic_tokens(paths.into_iter())
      .collect::<Vec<_>>();
    for expanded in expanded.iter() {
      let expanded = expanded.to_str()
        .expect("should always be utf8");
      assert!(!expanded.contains("PLATFORM"));
    }
  }

  #[test]
  fn multiple() {
    let paths = vec![
      PathBuf::from("$ORIGIN/../${LIB}"),
    ];
    let expanded = expand_dynamic_tokens(paths.into_iter())
      .collect::<Vec<_>>();
    for expanded in expanded.iter() {
      let expanded = expanded.to_str()
        .expect("should always be utf8");
      assert!(!expanded.contains("$"));
      assert!(!expanded.contains("ORIGIN"));
      assert!(!expanded.contains("LIB"));
      assert!(expanded.contains("/../"));
    }
  }
  #[test]
  fn prefix() {
    let paths = vec![
      PathBuf::from("/usr/${LIB}"),
    ];
    let expanded = expand_dynamic_tokens(paths.into_iter())
      .collect::<Vec<_>>();
    for expanded in expanded.iter() {
      let expanded = expanded.to_str()
        .expect("should always be utf8");
      assert!(!expanded.contains("$"));
      assert!(!expanded.contains("LIB"));
      assert!(expanded.starts_with("/usr/"));
    }
  }
}
