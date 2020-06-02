use std::env;
use std::path::Path;

use ctl10n;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=locales/*.toml");
    if let Err(err) = ctl10n::convert_strings_file(
        format!(
            "locales/{}.toml",
            &env::var("LOCALE").unwrap_or("zh".to_string())
        ),
        Path::new(&env::var("OUT_DIR").unwrap()).join("ctl10n_macros.rs"),
    ) {
        panic!("{}", err);
    }
}
