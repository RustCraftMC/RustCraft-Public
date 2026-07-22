use super::App;
use crate::client;
use crate::client::book::BookEditor;
use winit::keyboard::KeyCode;

impl App {
    pub(super) fn open_writable_book(&mut self) {
        let selected = self.inventory.selected;
        let held = self.inventory.protocol_slot_for_selected_item();
        if held.item_id != 386 {
            return;
        }
        // Vanilla sends the use-item packet before opening GuiScreenBook.
        self.send_item_use_packet();
        self.book_editor = Some(BookEditor::from_nbt(selected, held.nbt.as_deref()));
        self.input_ctrl.mouse_captured = false;
        self.set_cursor_captured(false);
        if let Some(window) = &self.window {
            window.set_ime_allowed(true);
        }
    }

    pub(super) fn handle_book_key(&mut self, code: KeyCode, text: Option<&str>, ctrl: bool) {
        let Some(editor) = &mut self.book_editor else {
            return;
        };
        if editor.signing {
            match code {
                KeyCode::Escape => editor.signing = false,
                KeyCode::Enter if editor.can_sign() => self.submit_book_editor(true),
                KeyCode::Backspace => editor.backspace_title(),
                _ if text.is_some() => editor.insert_title(text.unwrap_or_default()),
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Escape => self.close_book_editor(),
            KeyCode::PageUp | KeyCode::ArrowLeft => editor.previous_page(),
            KeyCode::PageDown | KeyCode::ArrowRight => editor.next_page(),
            KeyCode::Enter => editor.insert_text("\n"),
            KeyCode::Backspace => editor.backspace(),
            KeyCode::KeyV if ctrl => {
                if let Some(text) = super::input::get_clipboard_text() {
                    editor.insert_text(&text);
                }
            }
            _ if text.is_some() => editor.insert_text(text.unwrap_or_default()),
            _ => {}
        }
    }

    pub(super) fn append_book_text(&mut self, text: &str) {
        let Some(editor) = &mut self.book_editor else {
            return;
        };
        if editor.signing {
            editor.insert_title(text);
        } else {
            editor.insert_text(text);
        }
    }

    pub(super) fn handle_book_click(&mut self) {
        let hit = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.gui_hit_test(self.input_ctrl.mouse_x as f32, self.input_ctrl.mouse_y as f32));
        let Some(id) = hit else {
            return;
        };
        let Some(editor) = &mut self.book_editor else {
            return;
        };
        match id {
            crate::ui::button_ids::BOOK_PREVIOUS_PAGE => editor.previous_page(),
            crate::ui::button_ids::BOOK_NEXT_PAGE => editor.next_page(),
            crate::ui::button_ids::BOOK_SIGN => editor.signing = true,
            crate::ui::button_ids::BOOK_CANCEL_SIGN => editor.signing = false,
            crate::ui::button_ids::BOOK_FINALIZE if editor.can_sign() => {
                self.submit_book_editor(true)
            }
            crate::ui::button_ids::BOOK_DONE => self.submit_book_editor(false),
            _ => {}
        }
    }

    fn close_book_editor(&mut self) {
        self.book_editor = None;
        self.input_ctrl.mouse_captured = true;
        self.set_cursor_captured(true);
        if let Some(window) = &self.window {
            window.set_ime_allowed(false);
        }
    }

    fn submit_book_editor(&mut self, signed: bool) {
        let Some(editor) = self.book_editor.take() else {
            return;
        };
        if signed && !editor.can_sign() {
            self.book_editor = Some(editor);
            return;
        }
        if editor.modified {
            let nbt = editor.nbt_payload(signed, &self.username);
            let slot = editor.slot;
            self.inventory.slot_meta[slot].nbt = Some(nbt);
            if signed {
                self.inventory.slots[slot].item_id = 387;
            }
            let stack = self.inventory.slots[slot]
                .to_protocol_slot_with_meta(Some(&self.inventory.slot_meta[slot]));
            client::network::send_book_update(&self.net_ctrl.connection, signed, &stack);
        }
        self.input_ctrl.mouse_captured = true;
        self.set_cursor_captured(true);
        if let Some(window) = &self.window {
            window.set_ime_allowed(false);
        }
    }
}
