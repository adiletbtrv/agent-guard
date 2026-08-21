use regex::Regex;

pub fn scan_secrets(content: &str) -> Result<(), String> {
    let checks = [
        ("AWS access key", Regex::new(r"\bAKIA[0-9A-Z]{16}\b")),
        (
            "AWS secret key",
            Regex::new(r"(?i)aws_secret_access_key\s*[:=]\s*[A-Za-z0-9/+=]{32,}"),
        ),
        (
            "JWT",
            Regex::new(r"\beyJ[a-zA-Z0-9_-]{8,}\.[a-zA-Z0-9_-]{8,}\.[a-zA-Z0-9_-]{8,}\b"),
        ),
        (
            "private key",
            Regex::new(r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
        ),
    ];
    for (label, pattern) in checks {
        let pattern =
            pattern.map_err(|error| format!("security scanner initialization failed: {error}"))?;
        if pattern.is_match(content) {
            return Err(format!("{label} detected"));
        }
    }

    let assignment =
        Regex::new(r#"(?im)^\s*(?:token|secret|password|api[_-]?key)\s*[:=]\s*["']?([^\s"']+)"#)
            .map_err(|error| format!("security scanner initialization failed: {error}"))?;
    for capture in assignment.captures_iter(content) {
        let value = capture
            .get(1)
            .map(|match_| match_.as_str())
            .unwrap_or_default();
        if value.len() >= 20 && entropy(value) >= 3.5 {
            return Err("high-entropy credential assignment detected".to_string());
        }
    }
    Ok(())
}

fn entropy(value: &str) -> f64 {
    let mut counts = [0usize; 256];
    for byte in value.bytes() {
        counts[byte as usize] += 1;
    }
    let length = value.len() as f64;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let probability = *count as f64 / length;
            -probability * probability.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::scan_secrets;

    #[test]
    fn detects_aws_key() {
        assert!(scan_secrets("AKIA1234567890ABCDEF").is_err());
    }

    #[test]
    fn accepts_normal_source() {
        assert!(scan_secrets("const message = 'hello';").is_ok());
    }
}
