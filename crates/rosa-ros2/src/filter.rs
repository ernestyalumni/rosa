//! Post-execution output filtering for ROS 2 CLI tools.
//!
//! Two filters applied in sequence:
//! 1. **Blacklist** — drop any line containing a blacklisted token (default:
//!    `["master", "docker"]` matching upstream rosa's convention).
//! 2. **Pattern**  — keep only lines that match an optional user-supplied regex.

use regex::Regex;

/// Filter lines from `ros2` stdout.
///
/// * `blacklist` — lines containing any token are dropped (case-sensitive substring match).
/// * `pattern`   — if `Some`, only lines matching the regex are kept; `None` keeps all.
///
/// Blank lines are always dropped.
///
/// # Examples
/// ```
/// use rosa_ros2::filter::filter_lines;
///
/// let output = "/rosout\n/docker_node\n/turtlesim\n";
/// let bl = vec!["docker".to_owned()];
/// let result = filter_lines(output, &bl, None);
/// assert_eq!(result, vec!["/rosout", "/turtlesim"]);
/// ```
pub fn filter_lines(output: &str, blacklist: &[String], pattern: Option<&str>) -> Vec<String> {
    let re: Option<Regex> = pattern.and_then(|p| Regex::new(p).ok());

    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !blacklist.iter().any(|bl| l.contains(bl.as_str())))
        .filter(|l| re.as_ref().map_or(true, |re| re.is_match(l)))
        .map(|l| l.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklist_removes_matching_lines() {
        let out = "/rosout\n/master_node\n/turtlesim\n/docker_bridge\n";
        let bl  = vec!["master".into(), "docker".into()];
        let got = filter_lines(out, &bl, None);
        assert_eq!(got, vec!["/rosout", "/turtlesim"]);
    }

    #[test]
    fn test_pattern_keeps_only_matching_lines() {
        let out = "/rosout\n/turtlesim\n/my_node\n";
        let got = filter_lines(out, &[], Some("turtle"));
        assert_eq!(got, vec!["/turtlesim"]);
    }

    #[test]
    fn test_blank_lines_dropped() {
        let out = "/a\n\n  \n/b\n";
        let got = filter_lines(out, &[], None);
        assert_eq!(got, vec!["/a", "/b"]);
    }

    #[test]
    fn test_empty_blacklist_keeps_all() {
        let out = "/a\n/b\n";
        let got = filter_lines(out, &[], None);
        assert_eq!(got, vec!["/a", "/b"]);
    }

    #[test]
    fn test_invalid_regex_is_ignored_all_kept() {
        let out = "/a\n/b\n";
        // "[invalid" is not a valid regex — should keep all lines
        let got = filter_lines(out, &[], Some("[invalid"));
        assert_eq!(got, vec!["/a", "/b"]);
    }
}
