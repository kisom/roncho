use roncho::models::page::Page;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Human,
    Json,
}

pub fn emit<T: serde::Serialize>(mode: Mode, human: &str, json: &T) {
    match mode {
        Mode::Human => println!("{}", human.trim_end()),
        Mode::Json => println!("{}", serde_json::to_string_pretty(json).unwrap()),
    }
}

pub fn page<T, F>(mode: Mode, page: &Page<T>, render: F)
where
    T: serde::Serialize,
    F: Fn(&T) -> String,
{
    let items: Vec<String> = page.items.iter().map(render).collect();
    let body = if items.is_empty() {
        "(no items)".to_string()
    } else {
        items.join("\n")
    };
    let footer = format!(
        "\n\n{} (page {} of {}, {} total)",
        page.total,
        page.page,
        page.pages.max(1),
        page.items.len()
    );
    emit(mode, &format!("{body}{footer}"), page);
}

pub fn json_map(map: &serde_json::Map<String, serde_json::Value>) -> String {
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max).collect();
        format!("{end}…")
    }
}
