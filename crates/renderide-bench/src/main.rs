//! `renderide-bench` front-door binary.

#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    renderide_bench::cli_main()
}
