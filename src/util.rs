use std::path::Path;

pub fn except_mdbook_common_resources(p: &Path) -> bool {
    match p.to_str() {
        Some("ayu-highlight.css") => false,
        Some("highlight.css") => false,
        Some("tomorrow-niht.css") => false,
        _ => {
            if let Some(parent) = p.parent() {
                !matches!(
                    parent.to_str(),
                    Some("/css") | Some("/FontAwesome") | Some("/fonts")
                )
            } else {
                true
            }
        }
    }
}
