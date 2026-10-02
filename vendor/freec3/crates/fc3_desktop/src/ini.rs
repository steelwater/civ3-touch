use std::collections::HashMap;

/// Parse a Windows-style INI file into sections of key-value pairs.
///
/// Returns a map of section name → (key → value).
/// Keys before any section header go into the empty-string section.
/// Keys and section names preserve their original casing.
/// Values are trimmed of surrounding whitespace.
pub fn parse_ini(input: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in input.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                current_section = line[1..end].to_string();
            }
            continue;
        }

        // Key=Value pair
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key, value);
        }
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_ini() {
        let input = r#"
[Speed]
Normal Speed=225
Fast Speed=225

[Animations]
DEFAULT=WarriorDefault.flc
RUN=WarriorRun.flc
BLANK=
"#;
        let sections = parse_ini(input);

        let speed = sections.get("Speed").unwrap();
        assert_eq!(speed.get("Normal Speed").unwrap(), "225");
        assert_eq!(speed.get("Fast Speed").unwrap(), "225");

        let anims = sections.get("Animations").unwrap();
        assert_eq!(anims.get("DEFAULT").unwrap(), "WarriorDefault.flc");
        assert_eq!(anims.get("RUN").unwrap(), "WarriorRun.flc");
        assert_eq!(anims.get("BLANK").unwrap(), "");
    }

    #[test]
    fn test_parse_warrior_ini() {
        let input = r#"[Speed]
Normal Speed=225
Fast Speed=225

[Animations]
BLANK=
DEFAULT=WarriorDefault.flc
WALK=
RUN=WarriorRun.flc
ATTACK1=WarriorAttackA.flc
ATTACK2=WarriorAttackB.flc

[Timing]
DEFAULT=0.500000
WALK=0.500000

[Sound Effects]
DEFAULT=
RUN=WarriorRun.amb

[Version]
VERSION=1
[Palette]
PALETTE=
"#;
        let sections = parse_ini(input);

        assert!(sections.contains_key("Speed"));
        assert!(sections.contains_key("Animations"));
        assert!(sections.contains_key("Timing"));
        assert!(sections.contains_key("Sound Effects"));
        assert!(sections.contains_key("Version"));
        assert!(sections.contains_key("Palette"));

        let anims = sections.get("Animations").unwrap();
        assert_eq!(anims.get("DEFAULT").unwrap(), "WarriorDefault.flc");
        assert_eq!(anims.get("ATTACK1").unwrap(), "WarriorAttackA.flc");
        assert_eq!(anims.get("WALK").unwrap(), "");
    }

    #[test]
    fn test_parse_empty_input() {
        let sections = parse_ini("");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_parse_comments_ignored() {
        let input = r#"
; This is a comment
# This too
[Section]
key=value
"#;
        let sections = parse_ini(input);
        let s = sections.get("Section").unwrap();
        assert_eq!(s.get("key").unwrap(), "value");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_keys_before_section() {
        let input = "orphan_key=orphan_value\n[Section]\nkey=value\n";
        let sections = parse_ini(input);
        let global = sections.get("").unwrap();
        assert_eq!(global.get("orphan_key").unwrap(), "orphan_value");
    }

    #[test]
    fn test_whitespace_trimmed() {
        let input = "[Section]\n  key  =  value with spaces  \n";
        let sections = parse_ini(input);
        let s = sections.get("Section").unwrap();
        assert_eq!(s.get("key").unwrap(), "value with spaces");
    }

    #[test]
    fn test_equals_in_value() {
        let input = "[Section]\nkey=a=b=c\n";
        let sections = parse_ini(input);
        let s = sections.get("Section").unwrap();
        assert_eq!(s.get("key").unwrap(), "a=b=c");
    }
}
