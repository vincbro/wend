/// What a config file says about the setting at `path`, the names from the top joined by dots,
/// for the comment that goes above it. None for a section whose settings say it all.
#[must_use]
pub fn describe(path: &str) -> Option<&'static str> {
    Some(match path {
        "keys" => {
            "Keys. A combo is the keys typed in a row: `gs`, `<C-x>` for control, `<A-x>` for alt,\n\
             `<esc>` `<enter>` `<tab>` `<backspace>` `<space>` for the named keys, `<lt>` for `<`\n\
             and `<leader>` for the leader key. An action is a name, or a list of names that run\n\
             in order. Bind a combo to `[]` to remove it. A combo can't also be the start of a\n\
             longer one, so unbind `gs` and `ge` before binding `g`."
        }
        "keys.leader" => "The key `<leader>` stands for in a combo.",
        "keys.normal" => "Keys in normal mode.",
        "keys.select" => "Keys in select mode, where motions extend the selection.",
        "keys.insert" => "Keys in insert mode. A character that isn't bound here types itself.",
        "scroll.margin" => "The rows kept between the cursor and the edge of the view.",
        "text.tab_width" => "The columns between tab stops, at least 1.",
        "line_numbers.min_digits" => {
            "The fewest digits the numbers make room for, so the text only moves past that many."
        }
        "line_numbers.padding" => "The blank columns between the numbers and the text.",
        _ => return None,
    })
}
