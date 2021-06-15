pub fn is_match(input: &str, pattern: &str) -> bool {
    let mut input = input.chars().rev().map(|c| c.to_ascii_lowercase());
    let mut pattern = pattern.chars().rev().map(|c| c.to_ascii_lowercase());

    let mut is_wildcard = false;

    while let Some(p) = pattern.next() {
        if let Some(i) = input.next() {
            if i == p {
                continue;
            } else if p == '*' {
                if i == '.' {
                    return false;
                }
                is_wildcard = true;
                break;
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    while let Some(i) = input.next() {
        if !is_wildcard {
            return false;
        } else {
            if i == '.' {
                return false;
            }
        }
    }
    true
}
