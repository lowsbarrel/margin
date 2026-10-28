use std::fs;
use std::sync::mpsc;
use std::time::Duration;

use margin_fs::NotifyWatcher;
use margin_kernel::Watcher;

#[test]
fn coalesces_quick_changes_into_one_batch_of_relative_paths() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = directory.path().canonicalize().expect("a canonical root");
    let watcher = NotifyWatcher::new(root.clone());
    let (sender, receiver) = mpsc::channel();
    let Ok(subscription) = watcher.watch(Box::new(move |paths| {
        let _sent = sender.send(paths);
    })) else {
        return;
    };

    fs::write(root.join("a.md"), b"a").expect("writing a note");
    fs::write(root.join("b.md"), b"b").expect("writing a note");
    fs::write(root.join(".hidden.md"), b"h").expect("writing a hidden file");

    let batch = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("a batch arrives");
    let mut names: Vec<String> = batch.iter().map(ToString::to_string).collect();
    names.sort();
    assert_eq!(names, ["a.md", "b.md"]);
    assert!(
        receiver.recv_timeout(Duration::from_millis(500)).is_err(),
        "the changes coalesce into one batch"
    );
    subscription.cancel();
}
