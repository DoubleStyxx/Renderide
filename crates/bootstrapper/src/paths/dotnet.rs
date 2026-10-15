//! `dotnet` runtime resolution for the Renderite Host.

use std::path::{Path, PathBuf};

/// Prefers bundled `dotnet-runtime` under the Resonite folder (`dotnet.exe` then `dotnet` on Windows),
/// else `dotnet` on `PATH`.
pub fn find_dotnet_for_host(resonite_dir: &Path) -> PathBuf {
    let candidates: Vec<PathBuf> = {
        #[cfg(windows)]
        {
            vec![
                resonite_dir.join("dotnet-runtime").join("dotnet.exe"),
                resonite_dir.join("dotnet-runtime").join("dotnet"),
            ]
        }
        #[cfg(not(windows))]
        {
            vec![resonite_dir.join("dotnet-runtime").join("dotnet")]
        }
    };
    for c in candidates {
        if c.exists() {
            return c;
        }
    }
    PathBuf::from("dotnet")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn find_dotnet_for_host_prefers_bundled_dotnet() {
        let tmp = env::temp_dir().join(format!("bootstrapper_dotnet_unix_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let bundled = tmp.join("dotnet-runtime").join("dotnet");
        fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        fs::write(&bundled, b"").unwrap();
        assert_eq!(find_dotnet_for_host(&tmp), bundled);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[cfg(windows)]
    #[test]
    fn find_dotnet_for_host_prefers_bundled_exe() {
        let tmp = env::temp_dir().join(format!("bootstrapper_dotnet_win_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let bundled_exe = tmp.join("dotnet-runtime").join("dotnet.exe");
        fs::create_dir_all(bundled_exe.parent().unwrap()).unwrap();
        fs::write(&bundled_exe, b"").unwrap();
        assert_eq!(find_dotnet_for_host(&tmp), bundled_exe);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_dotnet_for_host_falls_back_without_bundled() {
        let tmp = env::temp_dir().join(format!(
            "bootstrapper_dotnet_fallback_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        assert_eq!(find_dotnet_for_host(&tmp), PathBuf::from("dotnet"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
