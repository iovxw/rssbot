use std::env;
use std::path::Path;

use ctl10n;

const LOCALES: &[&str] = &["zh", "en"];

fn main() {
    for locale in LOCALES {
        println!("cargo:rerun-if-changed=locales/{}.toml", locale);
    }
    println!("cargo:rerun-if-env-changed=LOCALE");
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
