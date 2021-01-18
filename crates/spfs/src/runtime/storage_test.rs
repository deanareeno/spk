use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;

use rstest::{fixture, rstest};

use super::{ensure_runtime, makedirs_with_perms, Config, Runtime, Storage};
use crate::encoding;

#[fixture]
fn tmpdir() -> tempdir::TempDir {
    tempdir::TempDir::new_in("/tmp", module_path!().clone().replace("::", "_").as_ref())
        .expect("failed to create tempdir for test")
}

#[rstest]
#[tokio::test]
async fn test_config_serialization() {
    let expected = Config {
        stack: vec![encoding::NULL_DIGEST.into(), encoding::EMPTY_DIGEST.into()],
        ..Default::default()
    };
    let data = serde_json::to_string_pretty(&expected).expect("failed to serialize config");
    let actual: Config = serde_json::from_str(&data).expect("failed to deserialize config data");

    assert_eq!(actual, expected);
}

#[rstest]
#[tokio::test]
async fn test_runtime_properties(tmpdir: tempdir::TempDir) {
    let runtime = Runtime::new(tmpdir.path()).expect("failed to create runtime for test");
    assert_eq!(tmpdir.path(), runtime.root());
    assert_eq!(
        runtime.config_file.file_name(),
        Some(OsStr::new(Runtime::CONFIG_FILE))
    );
}

#[rstest]
#[tokio::test]
async fn test_runtime_config_notnone(tmpdir: tempdir::TempDir) {
    let mut runtime = Runtime::new(tmpdir.path()).expect("failed to create runtime for test");
    assert_eq!(runtime.config, Config::default());
    assert!(runtime.read_config().is_ok());
    assert!(runtime.config_file.metadata().is_ok());
}

#[rstest]
#[tokio::test]
async fn test_ensure_runtime(tmpdir: tempdir::TempDir) {
    let runtime = ensure_runtime(tmpdir.path().join("root")).expect("failed to ensure runtime");
    assert!(runtime.root().metadata().is_ok(), "root should exist");
    assert!(
        runtime.upper_dir.metadata().is_ok(),
        "upper_dir should exist"
    );

    ensure_runtime(runtime.root()).expect("failed to ensure runtime on second call");
}

#[rstest]
#[tokio::test]
async fn test_storage_create_runtime(tmpdir: tempdir::TempDir) {
    let storage = Storage::new(tmpdir.path()).expect("failed to create storage");

    let runtime = storage
        .create_runtime()
        .expect("failed to create runtime in storage");
    assert!(!runtime.reference().is_empty());
    assert!(runtime.root().metadata().unwrap().file_type().is_dir());

    assert!(storage.create_named_runtime(runtime.reference()).is_err());
}

#[rstest]
#[tokio::test]
async fn test_storage_remove_runtime(tmpdir: tempdir::TempDir) {
    let storage = Storage::new(tmpdir.path()).expect("failed to create storage");

    assert!(
        storage.remove_runtime("non-existant").is_err(),
        "should fail to remove non-existant runtime"
    );

    let runtime = storage.create_runtime().expect("failed to create runtime");
    storage
        .remove_runtime(runtime.reference())
        .expect("should remove runtime properly");
}

#[rstest]
#[tokio::test]
async fn test_storage_iter_runtimes(tmpdir: tempdir::TempDir) {
    let storage = Storage::new(tmpdir.path().join("root")).expect("failed to create storage");

    let runtimes: crate::Result<Vec<_>> = storage.iter_runtimes().collect();
    let runtimes = runtimes.expect("unexpected error while listing runtimes");
    assert_eq!(runtimes.len(), 0);

    storage.create_runtime().expect("failed to create runtime");
    let runtimes: crate::Result<Vec<_>> = storage.iter_runtimes().collect();
    let runtimes = runtimes.expect("unexpected error while listing runtimes");
    assert_eq!(runtimes.len(), 1);

    storage.create_runtime().expect("failed to create runtime");
    storage.create_runtime().expect("failed to create runtime");
    storage.create_runtime().expect("failed to create runtime");
    let runtimes: crate::Result<Vec<_>> = storage.iter_runtimes().collect();
    let runtimes = runtimes.expect("unexpected error while listing runtimes");
    assert_eq!(runtimes.len(), 4);
}

#[rstest]
#[tokio::test]
async fn test_runtime_reset(tmpdir: tempdir::TempDir) {
    let storage = Storage::new(tmpdir.path().join("root")).expect("failed to create storage");
    let mut runtime = storage
        .create_runtime()
        .expect("failed to create runtime in storage");
    let upper_dir = tmpdir.path().join("upper");
    runtime.upper_dir = upper_dir.clone();

    ensure(upper_dir.join("file"));
    ensure(upper_dir.join("dir/file"));
    ensure(upper_dir.join("dir/dir/dir/file"));
    ensure(upper_dir.join("dir/dir/dir/file2"));
    ensure(upper_dir.join("dir/dir/dir1/file"));
    ensure(upper_dir.join("dir/dir2/dir/file.other"));

    runtime
        .reset(&["file.*"])
        .expect("failed to reset runtime paths");
    assert!(!upper_dir.join("dir/dir2/dir/file.other").exists());
    assert!(upper_dir.join("dir/dir/dir/file2").exists());

    runtime
        .reset(&["dir1/"])
        .expect("failed to reset runtime paths");
    assert!(upper_dir.join("dir/dir/dir").exists());
    assert!(upper_dir.join("dir/dir2").exists());

    runtime
        .reset(&["/file"])
        .expect("failed to reset runtime paths");
    assert!(upper_dir.join("dir/dir/dir/file").exists());
    assert!(!upper_dir.join("file").exists());

    runtime.reset_all().expect("failed to reset runtime paths");
    assert_eq!(listdir(upper_dir), Vec::<String>::new());
}

#[rstest]
#[tokio::test]
async fn test_makedirs_dont_change_existing(tmpdir: tempdir::TempDir) {
    let chkdir = tmpdir.path().join("my_dir");
    ensure(chkdir.join("file"));
    std::fs::set_permissions(&chkdir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let original = std::fs::metadata(&chkdir).unwrap().permissions().mode();
    makedirs_with_perms(chkdir.join("new"), 0o777).expect("makedirs should not fail");
    let actual = std::fs::metadata(&chkdir).unwrap().permissions().mode();
    assert_eq!(actual, original, "existing dir should not change perms");
}

fn listdir(path: std::path::PathBuf) -> Vec<String> {
    std::fs::read_dir(path)
        .expect("failed to read dir")
        .into_iter()
        .map(|res| {
            res.expect("error while reading dir")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn ensure(path: std::path::PathBuf) {
    std::fs::create_dir_all(path.parent().unwrap()).expect("failed to make dirs");
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .expect("failed to create file");
}
