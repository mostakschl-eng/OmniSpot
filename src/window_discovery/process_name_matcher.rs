use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::OnceLock;
use windows::core::PCSTR;
use windows::Win32::Globalization::*;

thread_local! {
    static COLLATION: RefCell<HashMap<String, Option<Collator>>> = RefCell::new(HashMap::new());
}

struct Collator(*mut UCollator);
impl Drop for Collator {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ucol_close(self.0);
            }
        }
    }
}

struct Elements(*mut UCollationElements);
impl Drop for Elements {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ucol_closeElements(self.0);
            }
        }
    }
}

// Match .NET's IgnoreCase collation while retaining width and kana distinctions.
// Reference: dotnet/runtime v8.0.0, System.Globalization.Native/pal_collation.c.
pub(crate) fn ends_with(value: &str, suffix: &str, culture: &str) -> bool {
    if suffix.is_empty() {
        return true;
    }
    with_collator(culture, |collator| collator.ends_with(value, suffix))
}

pub(super) fn invariant_equal(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    with_collator("", |collator| {
        let left: Vec<_> = left.encode_utf16().collect();
        let right: Vec<_> = right.encode_utf16().collect();
        unsafe {
            ucol_strcoll(
                collator.0,
                left.as_ptr(),
                left.len() as i32,
                right.as_ptr(),
                right.len() as i32,
            ) == UCOL_EQUAL
        }
    })
}

fn with_collator(culture: &str, compare: impl FnOnce(&Collator) -> bool) -> bool {
    COLLATION.with(|cache| {
        let mut cache = cache.borrow_mut();
        let collator =
            cache
                .entry(culture.to_string())
                .or_insert_with(|| match Collator::new(culture) {
                    Ok(collator) => Some(collator),
                    Err(error) => {
                        crate::window_discovery::warn(error);
                        None
                    }
                });
        collator.as_ref().is_some_and(compare)
    })
}

impl Collator {
    fn new(culture: &str) -> Result<Self, String> {
        let locale = culture.split_once('_').map_or_else(
            || culture.to_string(),
            |(name, sort)| format!("{name}@collation={sort}"),
        );
        let locale = CString::new(locale).map_err(|e| e.to_string())?;
        let mut status = U_ZERO_ERROR;
        let base = Self(unsafe { ucol_open(PCSTR(locale.as_ptr().cast()), &mut status) });
        if status.0 > 0 || base.0.is_null() {
            return Err(format!(
                "Unable to open ICU culture {culture}: {}",
                status.0
            ));
        }
        let mut length = 0;
        let mut rules = Vec::new();
        unsafe {
            let original = ucol_getRules(base.0, &mut length);
            if length > 0 {
                rules.extend_from_slice(std::slice::from_raw_parts(original, length as usize));
            }
        }
        static CASE_RULES: OnceLock<Result<Vec<u16>, String>> = OnceLock::new();
        rules.extend_from_slice(
            CASE_RULES
                .get_or_init(case_rules)
                .as_ref()
                .map_err(Clone::clone)?,
        );
        status = U_ZERO_ERROR;
        let pointer = unsafe {
            ucol_openRules(
                rules.as_ptr(),
                rules.len() as i32,
                UCOL_DEFAULT,
                UCOL_SECONDARY,
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if status.0 > 0 || pointer.is_null() {
            return Err(format!(
                "Unable to configure ICU culture {culture}: {}",
                status.0
            ));
        }
        Ok(Self(pointer))
    }

    fn ends_with(&self, value: &str, suffix: &str) -> bool {
        let source: Vec<_> = value.encode_utf16().collect();
        let suffix: Vec<_> = suffix.encode_utf16().collect();
        let mut status = U_ZERO_ERROR;
        let pattern = Elements(unsafe {
            ucol_openElements(self.0, suffix.as_ptr(), suffix.len() as i32, &mut status)
        });
        if status.0 > 0 || pattern.0.is_null() {
            return false;
        }
        let text = Elements(unsafe {
            ucol_openElements(self.0, source.as_ptr(), source.len() as i32, &mut status)
        });
        if status.0 > 0 || text.0.is_null() {
            return false;
        }
        loop {
            let pattern_weight = previous_weight(pattern.0, &mut status);
            if pattern_weight == -1 {
                return status.0 <= 0;
            }
            let source_weight = previous_weight(text.0, &mut status);
            if status.0 > 0
                || source_weight == -1
                || (pattern_weight as u32 & 0xffffff00) != (source_weight as u32 & 0xffffff00)
            {
                return false;
            }
        }
    }
}

fn previous_weight(elements: *mut UCollationElements, status: &mut UErrorCode) -> i32 {
    loop {
        let weight = unsafe { ucol_previous(elements, status) };
        if weight != 0 || status.0 > 0 {
            return weight;
        }
    }
}

fn case_rules() -> Result<Vec<u16>, String> {
    let mut status = U_ZERO_ERROR;
    let nfd = unsafe { unorm2_getNFDInstance(&mut status) };
    let nfkc = unsafe { unorm2_getNFKCInstance(&mut status) };
    if status.0 > 0 || nfd.is_null() || nfkc.is_null() {
        return Err("Unable to initialize ICU normalization".to_string());
    }
    let mut rules = Vec::new();
    let mut decomposition = [0u16; 8];
    for hiragana in (0x3041..=0x3096).chain(0x309d..=0x309e) {
        let count = unsafe {
            unorm2_getRawDecomposition(nfd, hiragana, decomposition.as_mut_ptr(), 8, &mut status)
        };
        if count < 0 {
            append_rule(&mut rules, hiragana as u16, b'<', (hiragana + 0x60) as u16);
        }
    }
    for full in (0xff01..=0xff5e)
        .chain(0xffe0..=0xffe6)
        .chain(0xff61..=0xff9d)
        .chain(0xffa0..=0xffdc)
    {
        if full == 0xff3c {
            continue;
        }
        let count = unsafe {
            unorm2_getRawDecomposition(nfkc, full, decomposition.as_mut_ptr(), 8, &mut status)
        };
        if count == 1 {
            append_rule(&mut rules, decomposition[0], b'<', full as u16);
        }
    }
    for upper in 0xff21..=0xff3a {
        append_rule(&mut rules, upper + 0x20, b'=', upper);
    }
    append_rule(&mut rules, b'a' as u16, b'=', b'a' as u16);
    if status.0 > 0 {
        return Err(format!(
            "Unable to build ICU comparison rules: {}",
            status.0
        ));
    }
    Ok(rules)
}

fn append_rule(rules: &mut Vec<u16>, left: u16, relation: u8, right: u16) {
    rules.push(b'&' as u16);
    literal(rules, left);
    rules.push(relation as u16);
    literal(rules, right);
}

fn literal(rules: &mut Vec<u16>, character: u16) {
    if character == b'\'' as u16 {
        rules.extend_from_slice(&[character, character]);
        return;
    }
    rules.push(b'\'' as u16);
    rules.push(character);
    rules.push(b'\'' as u16);
}
