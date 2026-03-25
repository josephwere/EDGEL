use std::io::{self, IsTerminal, Write};

struct Lesson {
    key: &'static str,
    title: &'static str,
    body: &'static str,
}

const LESSONS: &[Lesson] = &[
    Lesson {
        key: "1",
        title: "Foundations",
        body: include_str!("../../docs/lessons/01-foundations.md"),
    },
    Lesson {
        key: "2",
        title: "UI Screens",
        body: include_str!("../../docs/lessons/02-ui.md"),
    },
    Lesson {
        key: "3",
        title: "Web + API",
        body: include_str!("../../docs/lessons/03-web-api.md"),
    },
    Lesson {
        key: "4",
        title: "Debugging",
        body: include_str!("../../docs/lessons/04-debugging.md"),
    },
];

pub fn handle_learn(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(selector) = args.get(1) {
        let lesson = lesson_by_selector(selector).ok_or_else(|| {
            format!(
                "unknown lesson `{selector}`. Available lessons: {}",
                lesson_catalog_inline()
            )
        })?;
        print_lesson(lesson);
        return Ok(());
    }

    if !io::stdin().is_terminal() {
        print_catalog();
        println!();
        print_lesson(&LESSONS[0]);
        return Ok(());
    }

    println!("EDGEL Learn");
    println!("Choose a lesson number, `list`, or `quit`.");
    print_catalog();

    let mut current = 0usize;
    let mut input = String::new();
    loop {
        print!("learn> ");
        io::stdout().flush()?;
        input.clear();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let command = input.trim();
        match command {
            "" => {
                print_lesson(&LESSONS[current]);
                println!("\nType `next`, `list`, a lesson number, or `quit`.");
            }
            "list" | "catalog" => print_catalog(),
            "next" => {
                current = (current + 1).min(LESSONS.len().saturating_sub(1));
                print_lesson(&LESSONS[current]);
            }
            "prev" | "back" => {
                current = current.saturating_sub(1);
                print_lesson(&LESSONS[current]);
            }
            "quit" | "exit" => break,
            other => {
                if let Some(lesson) = lesson_by_selector(other) {
                    current = LESSONS
                        .iter()
                        .position(|candidate| candidate.key == lesson.key)
                        .unwrap_or(current);
                    print_lesson(lesson);
                } else {
                    println!(
                        "Unknown lesson command. Use `list`, `next`, `prev`, 1-{}, or `quit`.",
                        LESSONS.len()
                    );
                }
            }
        }
    }

    Ok(())
}

fn lesson_by_selector(selector: &str) -> Option<&'static Lesson> {
    let selector = selector.trim().to_ascii_lowercase();
    LESSONS.iter().find(|lesson| {
        lesson.key == selector
            || lesson.title.to_ascii_lowercase() == selector
            || lesson
                .title
                .to_ascii_lowercase()
                .replace(" + ", "-")
                .replace(' ', "-")
                == selector
    })
}

fn print_catalog() {
    println!("Lessons:");
    for lesson in LESSONS {
        println!(" - {}. {}", lesson.key, lesson.title);
    }
}

fn lesson_catalog_inline() -> String {
    LESSONS
        .iter()
        .map(|lesson| format!("{}. {}", lesson.key, lesson.title))
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_lesson(lesson: &Lesson) {
    println!("\n=== Lesson {}: {} ===\n", lesson.key, lesson.title);
    println!("{}", lesson.body.trim());
}
