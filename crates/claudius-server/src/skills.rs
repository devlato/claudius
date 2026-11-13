//! The skills for AI assistants (`skills/` in the repository), built in so that they always match the claudius that
//! runs them: `claudius skills` and `claudius connect` write them out, and the MCP tool `guide` reads them to an
//! assistant that has none installed.

/// every file of every skill, by its path under `skills/`
pub const FILES: &[(&str, &str)] = &[
    ("claudius-songs/SKILL.md", include_str!("../../../skills/claudius-songs/SKILL.md")),
    ("claudius-songs/references/script.md", include_str!("../../../skills/claudius-songs/references/script.md")),
    ("claudius-songs/references/verdict.md", include_str!("../../../skills/claudius-songs/references/verdict.md")),
    ("claudius-songs/references/genres.md", include_str!("../../../skills/claudius-songs/references/genres.md")),
    ("claudius-songs/references/tools.md", include_str!("../../../skills/claudius-songs/references/tools.md")),
    ("claudius-remix/SKILL.md", include_str!("../../../skills/claudius-remix/SKILL.md")),
    ("claudius-setup/SKILL.md", include_str!("../../../skills/claudius-setup/SKILL.md")),
];

/// the guide's topics, with the path of each under `skills/`: a skill by its name without `claudius-` (songs, remix,
/// setup), a reference by its file's name (script, verdict, genres, tools)
pub fn topics() -> Vec<(String, &'static str)> {
    FILES
        .iter()
        .map(|(path, _)| {
            let topic = match path.split_once('/') {
                Some((skill, "SKILL.md")) => skill.trim_start_matches("claudius-"),
                _ => path.rsplit('/').next().unwrap_or(path).trim_end_matches(".md"),
            };
            (topic.to_string(), *path)
        })
        .collect()
}

/// what `guide` answers: the text of `topic`, or the topics when it names none of them
pub fn guide(topic: Option<&str>) -> String {
    let topics = topics();
    let wanted = topic
        .map(|t| t.trim().to_ascii_lowercase())
        .map(|t| t.trim_start_matches("claudius-").trim_end_matches(".md").to_string())
        .filter(|t| !t.is_empty());
    let Some(wanted) = wanted else { return listed(&topics) };
    let Some((_, path)) = topics.iter().find(|(t, _)| *t == wanted) else {
        return format!("There is no topic {wanted}. {}", listed(&topics));
    };
    let (_, text) = FILES.iter().find(|(p, _)| p == path).copied().unwrap_or_default();
    match front_matter(text) {
        Some((_, body)) => format!(
            "(The files this names under references/ are topics of guide too: references/script.md is guide with topic \
             script.)\n\n{}",
            body.trim_start()
        ),
        None => text.to_string(),
    }
}

/// a skill's front matter, which says its name and when to use it, and the text after it
fn front_matter(text: &str) -> Option<(&str, &str)> {
    text.strip_prefix("---\n").and_then(|t| t.split_once("\n---\n"))
}

/// the topics, each with the first sentence of its skill's description or its reference's heading
fn listed(topics: &[(String, &str)]) -> String {
    let mut out =
        String::from("The claudius skills, built into this claudius; guide with one of these as topic reads it:\n");
    for (topic, path) in topics {
        let text = FILES.iter().find(|(p, _)| p == path).map_or("", |(_, t)| t);
        let what = match front_matter(text) {
            Some((front, _)) => {
                let description = front.split_once("description:").map_or("", |(_, d)| d);
                let words = description.split_whitespace().filter(|w| *w != ">-").collect::<Vec<_>>().join(" ");
                let first = words.split_once(". ").map_or(words.as_str(), |(s, _)| s).trim_end_matches('.');
                format!("{first}.")
            }
            None => {
                let heading = text.lines().next().unwrap_or_default().trim_start_matches('#').trim();
                let skill = path.split('/').next().unwrap_or_default().trim_start_matches("claudius-");
                format!("{heading} (from the {skill} skill).")
            }
        };
        out.push_str(&format!("\n- {topic}: {what}"));
    }
    out.push_str("\n\nRead songs first; it says when each of the others helps.");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_a_topic_of_its_own_and_the_list_names_them_all() {
        let topics = topics();
        let mut names: Vec<&str> = topics.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(names, ["songs", "script", "verdict", "genres", "tools", "remix", "setup"]);
        names.dedup();
        assert_eq!(names.len(), FILES.len());
        let list = guide(None);
        assert!(topics.iter().all(|(t, _)| list.contains(&format!("\n- {t}: "))), "{list}");
        assert!(list.contains("- songs: Makes and edits songs with claudius"), "{list}");
        assert!(list.contains("- script: The claudius script language, briefly (from the songs skill)."), "{list}");
    }

    #[test]
    fn a_topic_reads_its_text_without_the_front_matter_by_any_of_its_names() {
        let songs = guide(Some("songs"));
        assert!(songs.starts_with("(The files this names under references/ are topics of guide too"), "{songs:.200}");
        assert!(!songs.contains("\nname: claudius-songs\n") && songs.contains("\n# "), "{songs:.400}");
        assert_eq!(guide(Some(" Claudius-Remix ")), guide(Some("remix")));
        assert_eq!(guide(Some("script.md")), FILES[1].1);
        assert_eq!(guide(Some("")), guide(None));
        let unknown = guide(Some("mastering"));
        assert!(unknown.starts_with("There is no topic mastering. The claudius skills"), "{unknown:.200}");
    }
}
