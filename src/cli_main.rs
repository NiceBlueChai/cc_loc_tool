fn main() {
    if let Err(e) = cc_loc_tool::cli::run_cli() {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}
