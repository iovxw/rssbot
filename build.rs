use ctl10n;
use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=locales/*.toml");
    if let Err(err) = ctl10n::convert_strings_file(
        format!(
            "locales/{}.toml",
            &env::var("LOCALE").unwrap_or("zh".to_string())
        ),
        "ctl10n_macros.rs",
    ) {
        panic!("{}", err);
    }
}
