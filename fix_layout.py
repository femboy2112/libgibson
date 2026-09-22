import re

with open("src/layout.rs", "r") as f:
    text = f.read()

text = text.replace(
    "Dimension::Percent(p) => taffy::style::Dimension::Percent(*p),",
    "Dimension::Percent(p) => taffy::style::Dimension::Percent(*p / 100.0),"
)
text = text.replace(
    "Dimension::Percent(p) => LengthPercentageAuto::Percent(*p),",
    "Dimension::Percent(p) => LengthPercentageAuto::Percent(*p / 100.0),"
)
text = text.replace(
    "Dimension::Percent(p) => LengthPercentage::Percent(*p),",
    "Dimension::Percent(p) => LengthPercentage::Percent(*p / 100.0),"
)

with open("src/layout.rs", "w") as f:
    f.write(text)
