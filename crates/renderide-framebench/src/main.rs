//! `renderide-framebench` binary.

#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    renderide_framebench::cli_main()
}
