use std::ops::{Deref, DerefMut};
pub struct CachedCommand {
    command: assert_cmd::Command,
    _cache: tempfile::TempDir,
}
impl Deref for CachedCommand {
    type Target = assert_cmd::Command;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}
impl DerefMut for CachedCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}
pub fn ctx() -> CachedCommand {
    let cache = tempfile::tempdir().unwrap();
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("ctx");
    command.env("CONTEXT_ENGINE_CACHE_DIR", cache.path());
    CachedCommand {
        command,
        _cache: cache,
    }
}
