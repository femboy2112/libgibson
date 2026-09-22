with open("src/node.rs", "r") as f:
    text = f.read()

old = """    pub fn text_input(
        value: &str,
        cursor_grapheme: usize,
        placeholder: Option<&str>,
        style: Style,
    ) -> Self {
        Self {
            kind: NodeKind::TextInput {
                value: value.to_owned(),
                cursor_grapheme,
                placeholder: placeholder.map(|s| s.to_owned()),
                style,
                placeholder_style: Style::new().fg(Color::Rgb(100, 100, 100)),
                cursor_style: Style::new().reverse(),
                scroll_offset: 0,
            },
            layout_style: LayoutStyle::default(),
            computed_rect: Rect::default(),
            children: Vec::new(),
        }
    }"""

new = """    pub fn text_input(
        value: &str,
        cursor_grapheme: usize,
        placeholder: Option<&str>,
        style: Style,
    ) -> Self {
        Self {
            kind: NodeKind::TextInput {
                value: value.to_owned(),
                cursor_grapheme,
                placeholder: placeholder.map(|s| s.to_owned()),
                style,
                placeholder_style: Style::new().fg(Color::Rgb(100, 100, 100)),
                cursor_style: Style::new().reverse(),
                scroll_offset: 0,
            },
            layout_style: LayoutStyle::default(),
            computed_rect: Rect::default(),
            children: Vec::new(),
        }
    }

    pub fn scroll_offset(mut self, offset: usize) -> Self {
        if let NodeKind::TextInput { scroll_offset, .. } = &mut self.kind {
            *scroll_offset = offset;
        }
        self
    }"""

text = text.replace(old, new)
with open("src/node.rs", "w") as f:
    f.write(text)
