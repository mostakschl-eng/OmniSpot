pub(crate) struct WindowTitleParser;

impl WindowTitleParser {
    pub(crate) fn find_common_suffix(titles: &[String]) -> Option<&str> {
        let first = titles.first()?;
        if titles.len() <= 1 {
            return None;
        }
        let suffix = first
            .char_indices()
            .map(|(index, _)| &first[index..])
            .find(|suffix| titles[1..].iter().all(|title| title.ends_with(suffix)))?;
        [" - ", " — ", " | ", " : "]
            .into_iter()
            .find(|separator| {
                suffix.starts_with(separator) && suffix[separator.len()..].chars().count() >= 2
            })
            .map(|_| suffix)
    }

    pub(crate) fn strip_suffix<'a>(title: &'a str, suffix: Option<&str>) -> &'a str {
        match suffix.and_then(|suffix| title.strip_suffix(suffix)) {
            Some(stripped) => stripped.trim_end(),
            None => title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowTitleParser;

    #[test]
    fn matches_csharp_suffix_cases() {
        assert_eq!(WindowTitleParser::find_common_suffix(&[]), None);
        assert_eq!(
            WindowTitleParser::find_common_suffix(&["Only - App".into()]),
            None
        );
        assert_eq!(
            WindowTitleParser::find_common_suffix(&["Hello - X".into(), "World - X".into()]),
            None
        );
        assert_eq!(
            WindowTitleParser::find_common_suffix(&["Hello - é".into(), "World - é".into()]),
            None
        );
        assert_eq!(
            WindowTitleParser::find_common_suffix(&[
                "Document1.docx".into(),
                "Document2.docx".into()
            ]),
            None
        );
        for separator in [" - ", " — ", " | ", " : "] {
            let suffix = format!("{separator}MyApp");
            let titles = vec![format!("First{suffix}"), format!("Second{suffix}")];
            assert_eq!(
                WindowTitleParser::find_common_suffix(&titles),
                Some(suffix.as_str())
            );
            assert_eq!(
                WindowTitleParser::strip_suffix(&titles[0], Some(&suffix)),
                "First"
            );
        }
        assert_eq!(
            WindowTitleParser::strip_suffix("Hello - App", Some(" - Other")),
            "Hello - App"
        );
    }
}
