use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Keyword(Keyword),
    Identifier(String),
    String(String),
    Number(f64),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Gt,
    Gte,
    Lt,
    Lte,
    Range,
    Newline,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Import,
    Test,
    App,
    Screen,
    Text,
    Input,
    Button,
    Api,
    Db,
    Model,
    IdVerse,
    Let,
    Function,
    Async,
    Await,
    If,
    Else,
    Try,
    Catch,
    For,
    In,
    Return,
    Print,
    Connect,
    Table,
    Insert,
    Query,
    Where,
    Web,
    Page,
    Permissions,
    Type,
    True,
    False,
    And,
    Or,
    Optional,
    Header,
    P,
    H1,
    Scene,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer::new(source);
    lexer.lex_tokens()
}

struct Lexer<'a> {
    chars: Vec<char>,
    current: usize,
    line: usize,
    column: usize,
    _source: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            current: 0,
            line: 1,
            column: 1,
            _source: source,
        }
    }

    fn lex_tokens(&mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek() {
            let line = self.line;
            let column = self.column;
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                    tokens.push(Token {
                        kind: TokenKind::Newline,
                        line,
                        column,
                    });
                }
                '/' if self.peek_next() == Some('/') => {
                    while let Some(current) = self.peek() {
                        if current == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                '{' => tokens.push(self.single(TokenKind::LBrace)),
                '}' => tokens.push(self.single(TokenKind::RBrace)),
                '(' => tokens.push(self.single(TokenKind::LParen)),
                ')' => tokens.push(self.single(TokenKind::RParen)),
                '[' => tokens.push(self.single(TokenKind::LBracket)),
                ']' => tokens.push(self.single(TokenKind::RBracket)),
                ',' => tokens.push(self.single(TokenKind::Comma)),
                ':' => tokens.push(self.single(TokenKind::Colon)),
                '+' => tokens.push(self.single(TokenKind::Plus)),
                '-' => tokens.push(self.single(TokenKind::Minus)),
                '*' => tokens.push(self.single(TokenKind::Star)),
                '%' => tokens.push(self.single(TokenKind::Percent)),
                '.' => {
                    self.advance();
                    if self.match_char('.') {
                        tokens.push(Token {
                            kind: TokenKind::Range,
                            line,
                            column,
                        });
                    } else {
                        tokens.push(Token {
                            kind: TokenKind::Dot,
                            line,
                            column,
                        });
                    }
                }
                '=' => {
                    self.advance();
                    let kind = if self.match_char('=') {
                        TokenKind::EqEq
                    } else {
                        TokenKind::Eq
                    };
                    tokens.push(Token { kind, line, column });
                }
                '!' => {
                    self.advance();
                    let kind = if self.match_char('=') {
                        TokenKind::BangEq
                    } else {
                        TokenKind::Bang
                    };
                    tokens.push(Token { kind, line, column });
                }
                '>' => {
                    self.advance();
                    let kind = if self.match_char('=') {
                        TokenKind::Gte
                    } else {
                        TokenKind::Gt
                    };
                    tokens.push(Token { kind, line, column });
                }
                '<' => {
                    self.advance();
                    let kind = if self.match_char('=') {
                        TokenKind::Lte
                    } else {
                        TokenKind::Lt
                    };
                    tokens.push(Token { kind, line, column });
                }
                '/' => tokens.push(self.single(TokenKind::Slash)),
                '"' => tokens.push(self.string()?),
                ch if ch.is_ascii_digit() => tokens.push(self.number()?),
                ch if is_identifier_start(ch) => tokens.push(self.identifier()),
                _ => {
                    return Err(Diagnostic::new(
                        format!("unexpected character `{ch}`"),
                        line,
                        column,
                    ))
                }
            }
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            column: self.column,
        });
        Ok(tokens)
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let token = Token {
            kind,
            line: self.line,
            column: self.column,
        };
        self.advance();
        token
    }

    fn string(&mut self) -> Result<Token, Diagnostic> {
        let line = self.line;
        let column = self.column;
        self.advance();
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Ok(Token {
                    kind: TokenKind::String(value),
                    line,
                    column,
                });
            }
            value.push(ch);
            self.advance();
        }
        Err(Diagnostic::new("unterminated string", line, column))
    }

    fn number(&mut self) -> Result<Token, Diagnostic> {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        if self.peek() == Some('.') && self.peek_next().is_some_and(|next| next.is_ascii_digit()) {
            value.push('.');
            self.advance();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    value.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
        }
        let number = value.parse::<f64>().map_err(|_| {
            Diagnostic::new(format!("invalid number literal `{value}`"), line, column)
        })?;
        Ok(Token {
            kind: TokenKind::Number(number),
            line,
            column,
        })
    }

    fn identifier(&mut self) -> Token {
        let line = self.line;
        let column = self.column;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if is_identifier_continue(ch) {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match keyword_for(&value) {
            Some(keyword) => TokenKind::Keyword(keyword),
            None => TokenKind::Identifier(value),
        };

        Token { kind, line, column }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.current).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.current + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.current += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn keyword_for(value: &str) -> Option<Keyword> {
    Some(match value {
        "app" => Keyword::App,
        "import" => Keyword::Import,
        "test" => Keyword::Test,
        "screen" => Keyword::Screen,
        "text" => Keyword::Text,
        "input" => Keyword::Input,
        "button" => Keyword::Button,
        "api" => Keyword::Api,
        "db" => Keyword::Db,
        "model" => Keyword::Model,
        "idverse" => Keyword::IdVerse,
        "let" => Keyword::Let,
        "function" => Keyword::Function,
        "async" => Keyword::Async,
        "await" => Keyword::Await,
        "if" => Keyword::If,
        "else" => Keyword::Else,
        "try" => Keyword::Try,
        "catch" => Keyword::Catch,
        "for" => Keyword::For,
        "in" => Keyword::In,
        "return" => Keyword::Return,
        "print" => Keyword::Print,
        "connect" => Keyword::Connect,
        "table" => Keyword::Table,
        "insert" => Keyword::Insert,
        "query" => Keyword::Query,
        "where" => Keyword::Where,
        "web" => Keyword::Web,
        "page" => Keyword::Page,
        "permissions" => Keyword::Permissions,
        "type" => Keyword::Type,
        "true" => Keyword::True,
        "false" => Keyword::False,
        "and" => Keyword::And,
        "or" => Keyword::Or,
        "optional" => Keyword::Optional,
        "header" => Keyword::Header,
        "p" => Keyword::P,
        "h1" => Keyword::H1,
        "scene" => Keyword::Scene,
        _ => return None,
    })
}
