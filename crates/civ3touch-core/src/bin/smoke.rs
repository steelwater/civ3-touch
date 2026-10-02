fn main() {
    let rules = std::env::args_os()
        .nth(1)
        .expect("usage: smoke RULES_DIRECTORY");
    match civ3touch_core::smoke_test(std::path::Path::new(&rules)) {
        Ok(turn) => println!("PASS: FreeC3 rules, commands and replay; turn={turn}"),
        Err(error) => {
            eprintln!("FAIL: {error}");
            std::process::exit(1);
        }
    }
}
