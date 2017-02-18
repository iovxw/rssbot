use quick_xml::error::Error as XmlError;
error_chain! {
    errors {
        Unknown
    }
    foreign_links {
        Xml(XmlError);
    }
}
