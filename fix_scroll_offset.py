import re

with open("src/node.rs", "r") as f:
    text = f.read()

# Add scroll_offset builder method
text = re.sub(
    r"    pub fn text_input\([\s\S]*?\}",
    r"    pub fn text_input(\n        value: &str,\n        cursor_grapheme: usize,\n        placeholder: Option<&str>,\n        style: Style,\n    ) -> Self {\n        Self {\n            kind: NodeKind::TextInput {\n                value: value.to_owned(),\n                cursor_grapheme,\n                placeholder: placeholder.map(|s| s.to_owned()),\n                style,\n                placeholder_style: Style::new().fg(Color::Rgb(100, 100, 100)),\n                cursor_style: Style::new().reverse(),\n                scroll_offset: 0,\n            },\n            layout_style: LayoutStyle::default(),\n            computed_rect: Rect::default(),\n            children: Vec::new(),\n        }\n    }\n\n    pub fn scroll_offset(mut self, offset: usize) -> Self {\n        if let NodeKind::TextInput { scroll_offset, .. } = &mut self.kind {\n            *scroll_offset = offset;\n        }\n        self\n    }",
    text,
    count=1
)

with open("src/node.rs", "w") as f:
    f.write(text)
