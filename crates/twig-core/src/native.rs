use anyhow::Result;
#[cfg(not(target_os = "macos"))]
use anyhow::bail;
use std::{ffi::OsString, path::Path};

#[cfg(target_os = "macos")]
pub fn clone_file(source: &Path, destination: &Path) -> Result<()> {
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
pub fn clone_file(_: &Path, _: &Path) -> Result<()> {
    bail!("native cloning requires macOS")
}

pub fn set_executable(path: &Path, executable: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::{fs, os::unix::fs::PermissionsExt};
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
        )?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, executable);
    }
    Ok(())
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
