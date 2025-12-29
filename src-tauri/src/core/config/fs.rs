use std::fs;
use std::io;
use std::path::Path;

/// Abstract filesystem interface to allow mocking I/O operations in tests.
pub trait FileSystem: Send + Sync {
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;
    fn exists(&self, path: &Path) -> bool;
}

#[derive(Clone, Default)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        fs::copy(from, to)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
