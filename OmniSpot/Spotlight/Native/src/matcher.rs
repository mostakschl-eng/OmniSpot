use crate::model::{Candidate, Match};

pub(crate) fn rank_candidates(query: &str, candidates: &[Candidate], max_results: usize) -> Vec<Match> {
    let query = normalize(query);
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<Match> = candidates
        .iter()
        .filter_map(|candidate| {
            if !contains_required_characters(&query, candidate.character_mask) {
                return None;
            }
            let title_score = if candidate.normalized_title.is_empty() {
                score_text(&query, &candidate.title)
            } else {
                score_normalized(&query, &candidate.normalized_title)
            };
            let alternate_score = if candidate.normalized_alternate_name.is_empty() {
                score_text(&query, &candidate.alternate_name)
            } else {
                score_normalized(&query, &candidate.normalized_alternate_name)
            };
            let fuzzy_score = title_score.max(alternate_score)?;
            Some(Match {
                id: candidate.id,
                score: fuzzy_score + candidate.base_score + candidate.usage_boost,
            })
        })
        .collect();

    matches.sort_unstable_by(|left, right| {
        right.score.cmp(&left.score).then_with(|| left.id.cmp(&right.id))
    });
    matches.truncate(max_results);
    matches
}

fn contains_required_characters(query: &str, candidate_mask: [u64; 2]) -> bool {
    let mut query_mask = [0u64; 2];
    for character in query.bytes() {
        if character.is_ascii_lowercase() {
            let bit = (character - b'a') as usize;
            query_mask[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    (query_mask[0] & !candidate_mask[0]) == 0 && (query_mask[1] & !candidate_mask[1]) == 0
}

fn score_text(query: &str, candidate: &str) -> Option<i32> {
    let normalized = normalize(candidate);
    score_normalized(query, &normalized)
}

fn score_normalized(query: &str, normalized: &str) -> Option<i32> {
    if normalized.is_empty() || query.is_empty() {
        return None;
    }

    if normalized == query {
        return Some(1_000);
    }
    if normalized.starts_with(query) {
        return Some(920);
    }

    let tokens: Vec<&str> = normalized.split(' ').filter(|token| !token.is_empty()).collect();
    if tokens.iter().any(|token| token.starts_with(query)) {
        return Some(880);
    }

    let query_tokens: Vec<&str> = query.split(' ').filter(|token| !token.is_empty()).collect();
    if query_tokens.len() > 1
        && query_tokens.iter().all(|query_token| {
            tokens.iter().any(|candidate_token| candidate_token.starts_with(query_token))
        })
    {
        return Some(840);
    }

    if normalized.contains(query) {
        return Some(800);
    }

    let initials: String = tokens.iter().filter_map(|token| token.chars().next()).collect();
    if initials.starts_with(query) {
        return Some(780);
    }

    // Very short scattered matches create noisy results. Flow and PowerToys
    // also require stronger evidence for short queries.
    if query.chars().count() < 3 {
        return None;
    }

    let query_chars: Vec<char> = query.chars().collect();
    let candidate_chars: Vec<char> = normalized.chars().collect();
    let mut query_index = 0;
    let mut first = None;
    let mut last = 0;
    let mut consecutive = 0;
    let mut best_consecutive = 0;

    for (index, character) in candidate_chars.iter().enumerate() {
        if query_index < query_chars.len() && *character == query_chars[query_index] {
            first.get_or_insert(index);
            last = index;
            query_index += 1;
            consecutive += 1;
            best_consecutive = best_consecutive.max(consecutive);
        } else {
            consecutive = 0;
        }
    }

    if query_index != query_chars.len() {
        return None;
    }

    let first = first?;
    let span = last - first + 1;
    let compactness = ((query_chars.len() * 100) / span.max(1)) as i32;
    let boundary_bonus = if first == 0 || normalized.as_bytes().get(first.wrapping_sub(1)) == Some(&b' ') {
        60
    } else {
        0
    };
    Some(400 + compactness + (best_consecutive as i32 * 12) + boundary_bonus)
}

fn normalize(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.trim().chars() {
        let separator = matches!(character, ' ' | '\t' | '_' | '-' | '.');
        if separator {
            pending_space = true;
            continue;
        }
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;
        normalized.extend(character.to_lowercase());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::score_text;

    #[test]
    fn ranks_exact_and_prefix_matches_above_fuzzy_matches() {
        assert!(score_text("chrome", "Chrome").unwrap() > score_text("chr", "Google Chrome").unwrap());
        assert!(score_text("gch", "Google Chrome").is_some());
    }

    #[test]
    fn rejects_short_scattered_matches() {
        assert!(score_text("ce", "Google Chrome").is_none());
        assert!(score_text("chr", "Google Chrome").is_some());
    }

    #[test]
    fn handles_separator_normalization_and_acronyms() {
        assert!(score_text("control panel", "Control_Panel").is_some());
        assert!(score_text("cp", "Control Panel").is_some());
    }
}
