use crate::net::nbt::parse_root;

pub const MAX_BOOK_PAGES: usize = 50;
pub const MAX_BOOK_PAGE_CHARS: usize = 256;
pub const MAX_BOOK_TITLE_CHARS: usize = 16;

#[derive(Clone, Debug)]
pub struct BookEditor {
    pub slot: usize,
    pub pages: Vec<String>,
    pub page: usize,
    pub signing: bool,
    pub title: String,
    pub modified: bool,
}

impl BookEditor {
    pub fn from_nbt(slot: usize, nbt: Option<&[u8]>) -> Self {
        let mut pages = nbt
            .and_then(|bytes| parse_root(bytes).ok())
            .and_then(|root| root.as_compound().cloned())
            .and_then(|root| root.get("pages").cloned())
            .and_then(|pages| pages.as_list().map(|pages| pages.to_vec()))
            .map(|pages| {
                pages
                    .into_iter()
                    .filter_map(|page| page.as_str().map(str::to_owned))
                    .take(MAX_BOOK_PAGES)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if pages.is_empty() {
            pages.push(String::new());
        }
        Self {
            slot,
            pages,
            page: 0,
            signing: false,
            title: String::new(),
            modified: false,
        }
    }

    pub fn current_page(&self) -> &str {
        self.pages.get(self.page).map(String::as_str).unwrap_or("")
    }

    pub fn insert_text(&mut self, text: &str) {
        let page = &mut self.pages[self.page];
        for ch in text.chars() {
            if ch != '\n' && ch != '\r' && ch.is_control() {
                continue;
            }
            if page.chars().count() >= MAX_BOOK_PAGE_CHARS {
                break;
            }
            page.push(ch);
            self.modified = true;
        }
    }

    pub fn backspace(&mut self) {
        if self.pages[self.page].pop().is_some() {
            self.modified = true;
        }
    }

    pub fn next_page(&mut self) {
        if self.page + 1 < self.pages.len() {
            self.page += 1;
        } else if self.pages.len() < MAX_BOOK_PAGES {
            self.pages.push(String::new());
            self.page += 1;
            self.modified = true;
        }
    }

    pub fn previous_page(&mut self) {
        self.page = self.page.saturating_sub(1);
    }

    pub fn insert_title(&mut self, text: &str) {
        for ch in text.chars() {
            if ch.is_control() || self.title.chars().count() >= MAX_BOOK_TITLE_CHARS {
                continue;
            }
            self.title.push(ch);
            self.modified = true;
        }
    }

    pub fn backspace_title(&mut self) {
        if self.title.pop().is_some() {
            self.modified = true;
        }
    }

    pub fn can_sign(&self) -> bool {
        !self.title.trim().is_empty()
    }

    pub fn nbt_payload(&self, signed: bool, author: &str) -> Vec<u8> {
        let mut pages = self.pages.clone();
        while pages.len() > 1 && pages.last().is_some_and(|page| page.is_empty()) {
            pages.pop();
        }
        if signed {
            pages = pages
                .into_iter()
                .map(|page| serde_json::json!({ "text": page }).to_string())
                .collect();
        }
        encode_book_nbt(
            &pages,
            signed.then_some(self.title.trim()),
            signed.then_some(author),
        )
    }
}

fn encode_book_nbt(pages: &[String], title: Option<&str>, author: Option<&str>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(10); // TAG_Compound root
    write_nbt_string(&mut bytes, "");
    bytes.push(9); // TAG_List pages
    write_nbt_string(&mut bytes, "pages");
    bytes.push(8); // TAG_String list entries
    bytes.extend_from_slice(&(pages.len() as i32).to_be_bytes());
    for page in pages {
        write_nbt_string(&mut bytes, page);
    }
    if let Some(title) = title {
        write_named_string(&mut bytes, "title", title);
    }
    if let Some(author) = author {
        write_named_string(&mut bytes, "author", author);
    }
    bytes.push(0); // TAG_End
    bytes
}

fn write_named_string(bytes: &mut Vec<u8>, name: &str, value: &str) {
    bytes.push(8); // TAG_String
    write_nbt_string(bytes, name);
    write_nbt_string(bytes, value);
}

fn write_nbt_string(bytes: &mut Vec<u8>, value: &str) {
    let value = value.as_bytes();
    let length = value.len().min(u16::MAX as usize);
    bytes.extend_from_slice(&(length as u16).to_be_bytes());
    bytes.extend_from_slice(&value[..length]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_payload_round_trips_pages() {
        let editor = BookEditor {
            slot: 0,
            pages: vec!["First".to_string(), "Second".to_string()],
            page: 0,
            signing: false,
            title: String::new(),
            modified: true,
        };
        let reopened = BookEditor::from_nbt(0, Some(&editor.nbt_payload(false, "")));
        assert_eq!(reopened.pages, ["First", "Second"]);
    }
}
