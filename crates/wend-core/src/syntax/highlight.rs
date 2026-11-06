/// What a stretch of text is, as far as colors care. The frontend's theme turns it into a
/// color, the core never knows which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Highlight {
    Attribute,
    Comment,
    Constant,
    Constructor,
    Escape,
    Function,
    Keyword,
    Label,
    Operator,
    Property,
    Punctuation,
    String,
    Type,
    Variable,
}

impl Highlight {
    /// The highlight for a query capture name like `function.macro`. Only the part before
    /// the first dot counts for now, so `function.macro` is a `Function`. None for names
    /// that aren't about color, such as `injection.language`.
    #[must_use]
    pub fn from_capture(name: &str) -> Option<Self> {
        let kind = name.split_once('.').map_or(name, |(kind, _)| kind);
        Some(match kind {
            "attribute" => Self::Attribute,
            "comment" => Self::Comment,
            "constant" => Self::Constant,
            "constructor" => Self::Constructor,
            "escape" => Self::Escape,
            "function" => Self::Function,
            "keyword" => Self::Keyword,
            "label" => Self::Label,
            "operator" => Self::Operator,
            "property" => Self::Property,
            "punctuation" => Self::Punctuation,
            "string" => Self::String,
            "type" => Self::Type,
            "variable" => Self::Variable,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dotted_name_highlights_as_its_first_part() {
        assert_eq!(
            Highlight::from_capture("function.macro"),
            Some(Highlight::Function)
        );
        assert_eq!(
            Highlight::from_capture("punctuation.bracket"),
            Some(Highlight::Punctuation)
        );
        assert_eq!(
            Highlight::from_capture("keyword"),
            Some(Highlight::Keyword),
            "a name with no dot works too"
        );
    }

    #[test]
    fn names_about_something_else_than_color_highlight_nothing() {
        assert_eq!(Highlight::from_capture("injection.language"), None);
        assert_eq!(Highlight::from_capture("_hidden"), None);
    }
}
