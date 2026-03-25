use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub context: Option<String>,
    pub notes: Vec<String>,
    pub stack: Vec<String>,
    pub related: Vec<Diagnostic>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
            context: None,
            notes: Vec::new(),
            stack: Vec::new(),
            related: Vec::new(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_stack_frame(mut self, frame: impl Into<String>) -> Self {
        self.stack.push(frame.into());
        self
    }

    pub fn with_related(mut self, diagnostic: Diagnostic) -> Self {
        self.related.push(diagnostic);
        self
    }
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.context {
            Some(context) => write!(
                f,
                "line {}, column {}: {} ({})",
                self.line, self.column, self.message, context
            ),
            None => write!(f, "line {}, column {}: {}", self.line, self.column, self.message),
        }?;

        if !self.notes.is_empty() {
            write!(f, "\nnotes: {}", self.notes.join(" | "))?;
        }
        if !self.stack.is_empty() {
            write!(f, "\nstack: {}", self.stack.join(" -> "))?;
        }
        for related in &self.related {
            write!(
                f,
                "\nrelated: line {}, column {}: {}",
                related.line, related.column, related.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {}
