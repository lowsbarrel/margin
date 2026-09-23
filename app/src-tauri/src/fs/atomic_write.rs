use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn atomic_write(dest: &Path, content: &[u8]) -> Result<(), String> {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dest.with_file_name(format!(".margin-write-{n}.tmp"));

    let write_result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Failed to write temp file: {e}"));
    }

    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalize write: {e}")
    })?;

    // Opening or fsyncing a directory is unsupported on Windows, so the error is ignored there.
    if let Some(parent) = dest.parent()
        && let Ok(dir) = fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }

    Ok(())
}
