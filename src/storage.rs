//! Where the save text is kept: a file on desktop (see `vfield::save::file`), the browser's `localStorage` on the web.
//! `read` gives the saved text if there is any; `write` returns whether it was kept.

#[cfg(not(target_arch = "wasm32"))]
pub fn read() -> Option<String> {
    vfield::save::file::read_from(&vfield::save::file::default_dir(), std::path::Path::new("."))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write(text: &str) -> bool {
    vfield::save::file::write_to(&vfield::save::file::default_dir(), text)
}

// The three functions below are wasm imports from `env` (the `link` attribute: without it the linker treats them as
// undefined symbols and fails), provided by the plugin in `web/index.html`. A page without it gets miniquad's
// stand-ins, which return nothing: `vf_storage_len` then reads as 0 and `vf_storage_write` as a no-op, so the
// game still runs, it just does not remember.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    /// Length in bytes of the saved text, `-1` if there is none or storage is unavailable.
    fn vf_storage_len() -> i32;
    /// Copies the saved text into `ptr` (at most `cap` bytes); returns the bytes copied.
    fn vf_storage_read(ptr: *mut u8, cap: i32) -> i32;
    /// Stores `len` bytes at `ptr`; returns 1 if kept.
    fn vf_storage_write(ptr: *const u8, len: i32) -> i32;
}

#[cfg(target_arch = "wasm32")]
pub fn read() -> Option<String> {
    // SAFETY: the buffer is `n` bytes long and the plugin writes at most `cap` = `n` of them.
    unsafe {
        let n = vf_storage_len();
        if n <= 0 {
            return None;
        }
        let mut buf = vec![0u8; n as usize];
        let got = vf_storage_read(buf.as_mut_ptr(), n);
        buf.truncate(got.clamp(0, n) as usize);
        String::from_utf8(buf).ok()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn write(text: &str) -> bool {
    // SAFETY: `text` is valid for `len` bytes for the duration of the call.
    unsafe { vf_storage_write(text.as_ptr(), text.len() as i32) == 1 }
}
