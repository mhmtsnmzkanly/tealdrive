//! Development bootstrap for the protected home layout; the full Wizard is planned separately.
fn main() {
    if std::env::args().nth(1).as_deref() != Some("--initialize-protected-store") {
        eprintln!("Usage: init_accounts --initialize-protected-store (root; refuses overwrite)");
        std::process::exit(2);
    }
    match tealdrive::accounts::store::AccountStore::initialize(std::path::Path::new(
        tealdrive::layout::ACCOUNTS,
    )) {
        Ok(_) => println!("Protected account store initialized."),
        Err(error) => {
            eprintln!("Initialization refused: {error}");
            std::process::exit(1);
        }
    }
}
