use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) fn runtime_directory() -> PathBuf {
    let user_id = crate::unix_identity::effective_user_id();
    std::env::temp_dir().join(format!("native-system-monitor-{user_id}"))
}

pub(crate) fn socket_path() -> PathBuf {
    runtime_directory().join("agent.sock")
}

pub(crate) fn prepare_runtime_directory() -> io::Result<()> {
    ensure_private_directory(&runtime_directory())
}

pub(crate) fn ensure_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);

    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }

    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "local IPC runtime path is not a directory",
        ));
    }

    if metadata.uid() != crate::unix_identity::effective_user_id() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "local IPC runtime directory is owned by another user",
        ));
    }

    if metadata.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "local IPC runtime directory must have permissions 0700",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_private_directory;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);

    fn unique_test_path() -> PathBuf {
        let id = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "native-system-monitor-ipc-test-{}-{id}",
            std::process::id()
        ))
    }

    #[test]
    fn creates_a_private_runtime_directory_owned_by_the_current_user() {
        let path = unique_test_path();

        ensure_private_directory(&path).expect("create private runtime directory");

        let metadata = std::fs::symlink_metadata(&path).expect("runtime directory metadata");
        assert!(metadata.file_type().is_dir());
        assert_eq!(metadata.uid(), crate::unix_identity::effective_user_id());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);

        std::fs::remove_dir(path).expect("remove test runtime directory");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlink_as_the_runtime_directory() {
        use std::os::unix::fs::symlink;

        let path = unique_test_path();
        let target = unique_test_path();
        std::fs::create_dir(&target).expect("create symlink target");
        symlink(&target, &path).expect("create runtime directory symlink");

        let error = ensure_private_directory(&path).expect_err("reject runtime directory symlink");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        std::fs::remove_file(path).expect("remove runtime directory symlink");
        std::fs::remove_dir(target).expect("remove symlink target");
    }
}
