/// Shared text formatting helper for MC-style `%1$s`, `%2$s`, `%s`, `%d` placeholders.
///
/// Used by death screen, resource pack prompts, tooltips, and inventory labels.
pub fn format_text(template: &str, args: &[&str]) -> String {
    let mut out = template.to_string();
    for (idx, arg) in args.iter().enumerate() {
        out = out.replace(&format!("%{}$s", idx + 1), arg);
        out = out.replace(&format!("%{}$d", idx + 1), arg);
    }
    for arg in args {
        out = out.replacen("%s", arg, 1);
        out = out.replacen("%d", arg, 1);
    }
    out
}
