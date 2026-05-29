use std::io::{stdin, stdout};

fn main() {
    let result = tealdrive::helper::ipc::run_helper_once(&mut stdin().lock(), &mut stdout().lock());
    if result.is_err() {
        std::process::exit(1);
    }
}
