use crate::{debug, Result};

/// This is needed for opening lots of file descriptors, which tends to
/// happen more often when using RocksDB and making lots of federation
/// connections at startup. The soft limit is usually 1024, and the hard
/// limit is usually 512000; I've personally seen it hit >2000.
///
/// * <https://www.freedesktop.org/software/systemd/man/systemd.exec.html#id-1.12.2.1.17.6>
/// * <https://github.com/systemd/systemd/commit/0abf94923b4a95a7d89bc526efc84e7ca2b71741>
#[cfg(unix)]
pub fn maximize_fd_limit() -> Result<(), nix::errno::Errno> {
	use nix::sys::resource::{getrlimit, setrlimit, Resource::RLIMIT_NOFILE as NOFILE};

	let (soft_limit, hard_limit) = getrlimit(NOFILE)?;
	if soft_limit < hard_limit {
		setrlimit(NOFILE, hard_limit, hard_limit)?;
		assert_eq!((hard_limit, hard_limit), getrlimit(NOFILE)?, "getrlimit != setrlimit");
		debug!(to = hard_limit, from = soft_limit, "Raised RLIMIT_NOFILE",);
	}

	Ok(())
}

/// Get the number of threads which could execute in parallel based on the
/// hardware and administrative constraints of this system. This value should be
/// used to hint the size of thread-pools and divide-and-conquer algorithms.
///
/// * <https://doc.rust-lang.org/std/thread/fn.available_parallelism.html>
#[must_use]
pub fn available_parallelism() -> usize {
	std::thread::available_parallelism()
		.expect("Unable to query for available parallelism.")
		.get()
}

/// Return a possibly corrected std::env::current_exe() even if the path is
/// marked deleted.
///
/// # Safety
/// This function is declared unsafe because the original result was altered for
/// security purposes, and altering it back ignores those urposes and should be
/// understood by the user.
pub unsafe fn current_exe() -> Result<std::path::PathBuf> {
	use std::path::PathBuf;

	let exe = std::env::current_exe()?;
	match exe.to_str() {
		None => Ok(exe),
		Some(str) => Ok(str
			.strip_suffix(" (deleted)")
			.map(PathBuf::from)
			.unwrap_or(exe)),
	}
}

/// Determine if the server's executable was removed or replaced. This is a
/// specific check; useful for successful restarts. May not be available or
/// accurate on all platforms; defaults to false.
#[must_use]
pub fn current_exe_deleted() -> bool {
	std::env::current_exe().map_or(false, |exe| {
		exe.to_str()
			.map_or(false, |exe| exe.ends_with(" (deleted)"))
	})
}
