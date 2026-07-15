pub fn extract(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let rhs = trimmed
        .split_once(": ")
        .map(|(_, rhs)| rhs)
        .unwrap_or(trimmed);

    if let Some(percent) = rhs.split_whitespace().find(|token| token.ends_with('%')) {
        return Some(percent.to_string());
    }

    if let Some((done, total)) = extract_size_ratio(rhs)
        && total > 0.0
    {
        let phase = if rhs.contains("Extracting") {
            "Extracting"
        } else {
            "Downloading"
        };
        let percent = ((done / total) * 100.0).round().clamp(0.0, 100.0) as u8;
        return Some(format!("{} {}%", phase, percent));
    }

    for keyword in [
        "Waiting",
        "Pulling fs layer",
        "Downloading",
        "Extracting",
        "Download complete",
        "Pull complete",
        "Already exists",
    ] {
        if rhs.contains(keyword) {
            return Some(keyword.to_string());
        }
    }

    None
}

fn extract_size_ratio(text: &str) -> Option<(f64, f64)> {
    for token in text.split_whitespace() {
        let cleaned =
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '/');
        if let Some((left, right)) = cleaned.split_once('/') {
            let done = parse_size_to_bytes(left)?;
            let total = parse_size_to_bytes(right)?;
            return Some((done, total));
        }
    }

    None
}

fn parse_size_to_bytes(token: &str) -> Option<f64> {
    let cleaned = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
    if cleaned.is_empty() {
        return None;
    }

    let mut split_idx = cleaned.len();
    for (idx, ch) in cleaned.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.') {
            split_idx = idx;
            break;
        }
    }

    let number = cleaned[..split_idx].parse::<f64>().ok()?;
    let unit = cleaned[split_idx..].to_ascii_lowercase();

    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "tb" => 1_000_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        "tib" => 1_099_511_627_776.0,
        _ => return None,
    };

    Some(number * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_returns_existing_percent_token() {
        assert_eq!(extract("layer: Downloading 42%"), Some("42%".to_string()));
    }

    #[test]
    fn extract_converts_size_ratio_to_percent() {
        assert_eq!(
            extract("layer: Downloading 5MB/10MB"),
            Some("Downloading 50%".to_string())
        );
    }

    #[test]
    fn extract_reports_known_pull_phase() {
        assert_eq!(
            extract("layer: Pulling fs layer"),
            Some("Pulling fs layer".to_string())
        );
    }
}
