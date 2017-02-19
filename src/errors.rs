error_chain! {
    errors {
        Unknown

        AlreadySubscribed

        DatabaseOpen(path: String) {
            description("failed to open database")
            display("failed to open database: '{}'", path)
        }

        DatabaseSave(path: String) {
            description("failed to save database")
            display("failed to save database: '{}'", path)
        }

        DatabaseFormat {
            description("illegal database format")
        }
    }
    foreign_links {
        Xml(::quick_xml::error::Error);
    }
}
