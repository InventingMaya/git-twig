use anyhow::Result;
#[cfg(not(target_os = "macos"))]
use anyhow::bail;
use std::{ffi::OsString, path::Path};

#[cfg(target_os = "macos")]
pub fn clone_path(source: &Path, destination: &Path) -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // Darwin sys/clonefile.h; libc exposes clonefile but not these flag constants.
    const CLONE_NOFOLLOW: u32 = 0x0001;
    const CLONE_NOFOLLOW_ANY: u32 = 0x0008;
    // SAFETY: paths are live NUL-terminated strings; flags reject symlink traversal.
    let result = unsafe {
        libc::clonefile(
            source.as_ptr(),
            destination.as_ptr(),
            CLONE_NOFOLLOW | CLONE_NOFOLLOW_ANY,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn clone_path(_: &Path, _: &Path) -> Result<()> {
    bail!("native cloning requires macOS")
}

/// Discover effective creation permissions without changing the process-global umask.
pub struct FileModes {
    regular: std::fs::Permissions,
    executable: std::fs::Permissions,
}

impl FileModes {
    pub fn new(stage: &Path) -> Result<Self> {
        Ok(Self {
            regular: creation_permissions(&stage.join("regular-mode"), 0o666)?,
            executable: creation_permissions(&stage.join("executable-mode"), 0o777)?,
        })
    }

    pub fn apply_directory(&self, path: &Path) -> Result<()> {
        std::fs::set_permissions(path, self.executable.clone())?;
        Ok(())
    }

    pub fn apply(&self, path: &Path, executable: bool) -> Result<()> {
        let permissions = if executable {
            &self.executable
        } else {
            &self.regular
        };
        std::fs::set_permissions(path, permissions.clone())?;
        Ok(())
    }
}

fn creation_permissions(path: &Path, mode: u32) -> Result<std::fs::Permissions> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(options.open(path)?.metadata()?.permissions())
}

pub fn symlink(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (source, destination);
        bail!("symlinks require Unix")
    }
}

pub fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

pub fn os_string(bytes: Vec<u8>) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(bytes)
    }
    #[cfg(not(unix))]
    {
        OsString::from(String::from_utf8_lossy(&bytes).into_owned())
    }
}
