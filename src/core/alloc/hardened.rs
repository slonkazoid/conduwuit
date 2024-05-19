#[global_allocator]
static HMALLOC: hardened_malloc_rs::HardenedMalloc = hardened_malloc_rs::HardenedMalloc;

#[must_use]
pub fn memory_usage() -> String {
	String::default() //TODO: get usage
}

#[must_use]
pub fn memory_stats() -> String { "Extended statistics are not available from hardened_malloc.".to_owned() }
