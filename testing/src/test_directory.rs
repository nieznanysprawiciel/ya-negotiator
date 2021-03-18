use anyhow::Context;
use std::fs;
use std::path::PathBuf;

fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("test-workdir")
}

fn escape_path(path: &str) -> String {
    // Windows can't handle colons
    path.replace("::", "_").to_string()
}

pub fn prepare_test_dir(dir_name: &str) -> anyhow::Result<PathBuf> {
    let test_dir: PathBuf = test_data_dir().join(escape_path(dir_name).as_str());

    if test_dir.exists() {
        fs::remove_dir_all(&test_dir)
            .with_context(|| format!("Removing test directory: {}", test_dir.display()))?;
    }
    fs::create_dir_all(&test_dir)
        .with_context(|| format!("Creating test directory: {}", test_dir.display()))?;
    Ok(test_dir)
}
