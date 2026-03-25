use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};

pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    Parser::new(tokens).parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        let mut errors = Vec::new();
        self.skip_separators();
        while !self.is_at_end() {
            let parsed_item = if self.matches_keyword(Keyword::Import) {
                self.parse_import().map(Item::Import)
            } else if self.matches_keyword(Keyword::Test) {
                self.parse_test().map(Item::Test)
            } else if self.matches_keyword(Keyword::Async) {
                self.expect_keyword(Keyword::Function, "expected `function` after `async`")
                    .and_then(|_| self.parse_function(true))
                    .map(Item::Function)
            } else if self.matches_keyword(Keyword::Function) {
                self.parse_function(false).map(Item::Function)
            } else if self.matches_keyword(Keyword::App) {
                self.parse_app().map(Item::App)
            } else if self.matches_keyword(Keyword::Web) {
                self.parse_web().map(Item::Web)
            } else if self.matches_keyword(Keyword::Api) {
                self.parse_api().map(Item::Api)
            } else if self.matches_keyword(Keyword::Db) {
                self.parse_db().map(Item::Db)
            } else if self.matches_keyword(Keyword::Table) {
                self.parse_table().map(Item::Table)
            } else if self.matches_keyword(Keyword::Model) {
                self.parse_model().map(Item::Model)
            } else if self.matches_keyword(Keyword::IdVerse) {
                self.parse_idverse().map(Item::IdVerse)
            } else {
                self.parse_statement().map(Item::Statement)
            };

            match parsed_item {
                Ok(item) => items.push(item),
                Err(error) => {
                    errors.push(error);
                    self.synchronize_item();
                }
            }
            self.skip_separators();
        }
        if errors.is_empty() {
            Ok(Program { items })
        } else {
            Err(self.merge_diagnostics(errors))
        }
    }

    fn parse_import(&mut self) -> Result<ImportDecl, Diagnostic> {
        let module = match self.advance().kind.clone() {
            TokenKind::String(value) => value,
            TokenKind::Identifier(value) => {
                let mut module = value;
                while self.matches(&TokenKind::Dot) {
                    let segment = match self.advance().kind.clone() {
                        TokenKind::Identifier(value) => value,
                        TokenKind::Keyword(keyword) => self.keyword_name(keyword).to_string(),
                        _ => {
                            return Err(
                                self.error_previous("expected module segment after `.` in import")
                            )
                        }
                    };
                    module.push('.');
                    module.push_str(&segment);
                }
                module
            }
            TokenKind::Keyword(keyword) => {
                let mut module = self.keyword_name(keyword).to_string();
                while self.matches(&TokenKind::Dot) {
                    let segment = match self.advance().kind.clone() {
                        TokenKind::Identifier(value) => value,
                        TokenKind::Keyword(keyword) => self.keyword_name(keyword).to_string(),
                        _ => {
                            return Err(
                                self.error_previous("expected module segment after `.` in import")
                            )
                        }
                    };
                    module.push('.');
                    module.push_str(&segment);
                }
                module
            }
            _ => return Err(self.error_previous("expected module name after `import`")),
        };
        Ok(ImportDecl { module })
    }

    fn parse_function(&mut self, is_async: bool) -> Result<FunctionDecl, Diagnostic> {
        let name = self.expect_identifier("expected function name")?;
        self.expect(TokenKind::LParen, "expected `(` after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.expect_identifier("expected parameter name")?;
                let ty = if self.matches(&TokenKind::Colon) {
                    Some(self.expect_name("expected parameter type after `:`")?)
                } else {
                    None
                };
                params.push(Parameter {
                    name: param_name,
                    ty,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen, "expected `)` after parameters")?;
        let return_type = if self.matches(&TokenKind::Colon) {
            Some(self.expect_name("expected return type after `:`")?)
        } else {
            None
        };
        let body = self.parse_block_statements()?;
        Ok(FunctionDecl {
            is_async,
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_test(&mut self) -> Result<TestDecl, Diagnostic> {
        let name = match self.advance().kind.clone() {
            TokenKind::String(value) => value,
            TokenKind::Identifier(value) => value,
            _ => return Err(self.error_previous("expected test name after `test`")),
        };
        let body = self.parse_block_statements()?;
        Ok(TestDecl { name, body })
    }

    fn parse_app(&mut self) -> Result<AppDecl, Diagnostic> {
        let name = self.expect_identifier("expected app name")?;
        self.expect(TokenKind::LBrace, "expected `{` after app name")?;
        let mut screens = Vec::new();
        let mut permissions = Vec::new();
        let mut errors = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.matches_keyword(Keyword::Screen) {
                match self.parse_screen() {
                    Ok(screen) => screens.push(screen),
                    Err(error) => {
                        errors.push(error);
                        self.synchronize_block_item();
                    }
                }
            } else if self.matches_keyword(Keyword::Permissions) {
                match self.parse_permissions() {
                    Ok(parsed_permissions) => permissions = parsed_permissions,
                    Err(error) => {
                        errors.push(error);
                        self.synchronize_block_item();
                    }
                }
            } else {
                errors.push(self.error_here("expected `screen` or `permissions` inside app"));
                self.synchronize_block_item();
            }
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after app block")?;
        if !errors.is_empty() {
            return Err(self.merge_diagnostics(errors));
        }
        Ok(AppDecl {
            name,
            screens,
            permissions,
        })
    }

    fn parse_web(&mut self) -> Result<WebDecl, Diagnostic> {
        let name = self.expect_identifier("expected web app name")?;
        self.expect(TokenKind::LBrace, "expected `{` after web name")?;
        let mut pages = Vec::new();
        let mut apis = Vec::new();
        let mut errors = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.matches_keyword(Keyword::Page) {
                match self.parse_page() {
                    Ok(page) => pages.push(page),
                    Err(error) => {
                        errors.push(error);
                        self.synchronize_block_item();
                    }
                }
            } else if self.matches_keyword(Keyword::Api) {
                match self.parse_api() {
                    Ok(api) => apis.push(api),
                    Err(error) => {
                        errors.push(error);
                        self.synchronize_block_item();
                    }
                }
            } else {
                errors.push(self.error_here("expected `page` or `api` inside web block"));
                self.synchronize_block_item();
            }
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after web block")?;
        if !errors.is_empty() {
            return Err(self.merge_diagnostics(errors));
        }
        Ok(WebDecl { name, pages, apis })
    }

    fn parse_page(&mut self) -> Result<PageDecl, Diagnostic> {
        let route = self.expect_string("expected route string after `page`")?;
        let nodes = self.parse_ui_block()?;
        Ok(PageDecl { route, nodes })
    }

    fn parse_screen(&mut self) -> Result<ScreenDecl, Diagnostic> {
        let name = self.expect_identifier("expected screen name")?;
        let nodes = self.parse_ui_block()?;
        Ok(ScreenDecl { name, nodes })
    }

    fn parse_permissions(&mut self) -> Result<Vec<String>, Diagnostic> {
        self.expect(TokenKind::LBrace, "expected `{` after permissions")?;
        let mut permissions = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            permissions.push(self.expect_name("expected permission name")?);
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after permissions")?;
        Ok(permissions)
    }

    fn parse_ui_block(&mut self) -> Result<Vec<UiNode>, Diagnostic> {
        self.expect(TokenKind::LBrace, "expected `{`")?;
        let mut nodes = Vec::new();
        let mut errors = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            match self.parse_ui_node() {
                Ok(node) => nodes.push(node),
                Err(error) => {
                    errors.push(error);
                    self.synchronize_ui_node();
                }
            }
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after UI block")?;
        if !errors.is_empty() {
            return Err(self.merge_diagnostics(errors));
        }
        Ok(nodes)
    }

    fn parse_ui_node(&mut self) -> Result<UiNode, Diagnostic> {
        if self.matches_keyword(Keyword::Text) {
            Ok(UiNode::Text(self.parse_call_argument("text")?))
        } else if self.matches_keyword(Keyword::Header) || self.matches_keyword(Keyword::H1) {
            Ok(UiNode::Header(self.parse_call_argument("header")?))
        } else if self.matches_keyword(Keyword::P) {
            Ok(UiNode::Paragraph(self.parse_call_argument("p")?))
        } else if self.matches_keyword(Keyword::Input) {
            self.parse_input()
        } else if self.matches_keyword(Keyword::Button) {
            self.parse_button()
        } else if self.matches_keyword(Keyword::Scene) {
            self.parse_scene()
        } else {
            Err(self.error_here("expected a UI node like `text`, `input`, or `button`"))
        }
    }

    fn parse_input(&mut self) -> Result<UiNode, Diagnostic> {
        let name = self.expect_identifier("expected input name")?;
        let prompt = if self.matches(&TokenKind::LParen) {
            let expr = self.parse_expression()?;
            self.expect(TokenKind::RParen, "expected `)` after input prompt")?;
            Some(expr)
        } else {
            None
        };
        let input_type = if self.matches_keyword(Keyword::Type) {
            Some(self.expect_string("expected input type string")?)
        } else {
            None
        };
        Ok(UiNode::Input {
            name,
            prompt,
            input_type,
        })
    }

    fn parse_button(&mut self) -> Result<UiNode, Diagnostic> {
        let label = self.parse_call_argument("button")?;
        let actions = self.parse_block_statements()?;
        Ok(UiNode::Button { label, actions })
    }

    fn parse_scene(&mut self) -> Result<UiNode, Diagnostic> {
        self.expect(TokenKind::LBrace, "expected `{` after scene")?;
        let mut commands = Vec::new();
        let mut errors = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            match self.parse_scene_command() {
                Ok(command) => commands.push(command),
                Err(error) => {
                    errors.push(error);
                    self.synchronize_block_item();
                }
            }
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after scene block")?;
        if !errors.is_empty() {
            return Err(self.merge_diagnostics(errors));
        }
        Ok(UiNode::Scene { commands })
    }

    fn parse_scene_command(&mut self) -> Result<SceneCommand, Diagnostic> {
        let name = self.expect_name("expected scene command name")?;
        let label = if matches!(self.peek().kind, TokenKind::Identifier(_)) {
            Some(self.expect_identifier("expected scene label")?)
        } else {
            None
        };
        let args = if self.matches(&TokenKind::LParen) {
            self.parse_expression_list(TokenKind::RParen)?
        } else {
            Vec::new()
        };
        let children = if self.matches(&TokenKind::LBrace) {
            let mut children = Vec::new();
            while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                self.skip_separators();
                if self.check(&TokenKind::RBrace) {
                    break;
                }
                children.push(self.parse_scene_command()?);
                self.skip_separators();
            }
            self.expect(TokenKind::RBrace, "expected `}` after scene command block")?;
            children
        } else {
            Vec::new()
        };
        Ok(SceneCommand {
            name,
            label,
            args,
            children,
        })
    }

    fn parse_api(&mut self) -> Result<ApiDecl, Diagnostic> {
        let route = self.expect_string("expected route string after `api`")?;
        let body = self.parse_block_statements()?;
        Ok(ApiDecl { route, body })
    }

    fn parse_db(&mut self) -> Result<DbDecl, Diagnostic> {
        self.expect_keyword(Keyword::Connect, "expected `connect` after `db`")?;
        let name = self.expect_string("expected database name")?;
        Ok(DbDecl { name })
    }

    fn parse_table(&mut self) -> Result<TableDecl, Diagnostic> {
        let name = self.expect_identifier("expected table name")?;
        self.expect(TokenKind::LBrace, "expected `{` after table name")?;
        let mut columns = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            let column_name = self.expect_identifier("expected column name")?;
            self.expect(TokenKind::Colon, "expected `:` after column name")?;
            let ty = self.expect_name("expected column type")?;
            columns.push(TableColumn {
                name: column_name,
                ty,
            });
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after table block")?;
        Ok(TableDecl { name, columns })
    }

    fn parse_model(&mut self) -> Result<ModelDecl, Diagnostic> {
        let name = self.expect_identifier("expected model name")?;
        self.expect(TokenKind::LBrace, "expected `{` after model name")?;
        let mut properties = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            let property = self.expect_name("expected model property name")?;
            self.expect(TokenKind::Colon, "expected `:` after model property")?;
            let value = self.parse_expression()?;
            properties.push((property, value));
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after model block")?;
        Ok(ModelDecl { name, properties })
    }

    fn parse_idverse(&mut self) -> Result<IdVerseDecl, Diagnostic> {
        let name = self.expect_identifier("expected idverse object name")?;
        self.expect(TokenKind::LBrace, "expected `{` after idverse name")?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            let field_name = self.expect_name("expected idverse field")?;
            let optional = self.matches_keyword(Keyword::Optional);
            fields.push(IdVerseField {
                name: field_name,
                optional,
            });
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after idverse block")?;
        Ok(IdVerseDecl { name, fields })
    }

    fn parse_block_statements(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect(TokenKind::LBrace, "expected `{` to start block")?;
        let mut statements = Vec::new();
        let mut errors = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_separators();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            match self.parse_statement() {
                Ok(statement) => statements.push(statement),
                Err(error) => {
                    errors.push(error);
                    self.synchronize_statement();
                }
            }
            self.skip_separators();
        }
        self.expect(TokenKind::RBrace, "expected `}` after block")?;
        if errors.is_empty() {
            Ok(statements)
        } else {
            Err(self.merge_diagnostics(errors))
        }
    }

    fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        let line = self.peek().line;
        if self.matches_keyword(Keyword::Let) {
            let name = self.expect_identifier("expected variable name")?;
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.expect_name("expected variable type after `:`")?)
            } else {
                None
            };
            self.expect(TokenKind::Eq, "expected `=` after variable name")?;
            let expr = self.parse_expression()?;
            Ok(Stmt::Let {
                line,
                name,
                ty,
                expr,
            })
        } else if self.matches_keyword(Keyword::If) {
            let condition = self.parse_expression()?;
            let then_branch = self.parse_block_statements()?;
            let else_branch = if self.matches_keyword(Keyword::Else) {
                self.parse_block_statements()?
            } else {
                Vec::new()
            };
            Ok(Stmt::If {
                line,
                condition,
                then_branch,
                else_branch,
            })
        } else if self.matches_keyword(Keyword::Try) {
            let try_branch = self.parse_block_statements()?;
            self.expect_keyword(Keyword::Catch, "expected `catch` after `try` block")?;
            let error_name = self.expect_identifier("expected catch variable name")?;
            let catch_branch = self.parse_block_statements()?;
            Ok(Stmt::TryCatch {
                line,
                try_branch,
                error_name,
                catch_branch,
            })
        } else if self.matches_keyword(Keyword::For) {
            let name = self.expect_identifier("expected loop variable name")?;
            self.expect_keyword(Keyword::In, "expected `in` after loop variable")?;
            let start_or_iterable = self.parse_expression()?;
            if self.matches(&TokenKind::Range) {
                let end = self.parse_expression()?;
                let body = self.parse_block_statements()?;
                Ok(Stmt::ForRange {
                    line,
                    name,
                    start: start_or_iterable,
                    end,
                    body,
                })
            } else {
                let body = self.parse_block_statements()?;
                Ok(Stmt::ForEach {
                    line,
                    name,
                    iterable: start_or_iterable,
                    body,
                })
            }
        } else if self.matches_keyword(Keyword::Return) {
            if self.at_statement_end() {
                Ok(Stmt::Return { line, expr: None })
            } else {
                Ok(Stmt::Return {
                    line,
                    expr: Some(self.parse_expression()?),
                })
            }
        } else if self.matches_keyword(Keyword::Print) {
            Ok(Stmt::Print {
                line,
                expr: self.parse_call_argument("print")?,
            })
        } else if self.matches_keyword(Keyword::Insert) {
            let table = self.expect_identifier("expected table name after insert")?;
            let fields = self.parse_object_fields()?;
            Ok(Stmt::Insert { line, table, fields })
        } else if self.matches_keyword(Keyword::Query) {
            let table = self.expect_identifier("expected table name after query")?;
            let filter = if self.matches_keyword(Keyword::Where) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            Ok(Stmt::Query { line, table, filter })
        } else {
            Ok(Stmt::Expr {
                line,
                expr: self.parse_expression()?,
            })
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;
        while self.matches_keyword(Keyword::Or) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;
        while self.matches_keyword(Keyword::And) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.matches(&TokenKind::EqEq) {
                Some(BinaryOp::Equal)
            } else if self.matches(&TokenKind::BangEq) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };

            if let Some(op) = op {
                let right = self.parse_comparison()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.matches(&TokenKind::Gt) {
                Some(BinaryOp::Greater)
            } else if self.matches(&TokenKind::Gte) {
                Some(BinaryOp::GreaterEqual)
            } else if self.matches(&TokenKind::Lt) {
                Some(BinaryOp::Less)
            } else if self.matches(&TokenKind::Lte) {
                Some(BinaryOp::LessEqual)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_term()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches(&TokenKind::Minus) {
                Some(BinaryOp::Subtract)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_factor()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                Some(BinaryOp::Multiply)
            } else if self.matches(&TokenKind::Slash) {
                Some(BinaryOp::Divide)
            } else if self.matches(&TokenKind::Percent) {
                Some(BinaryOp::Modulo)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.matches(&TokenKind::Bang) {
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
            })
        } else if self.matches(&TokenKind::Minus) {
            Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_unary()?),
            })
        } else if self.matches_keyword(Keyword::Await) {
            Ok(Expr::Await(Box::new(self.parse_unary()?)))
        } else {
            self.parse_call()
        }
    }

    fn parse_call(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches(&TokenKind::LParen) {
                let args = self.parse_expression_list(TokenKind::RParen)?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
            } else if self.matches(&TokenKind::Dot) {
                let name = self.expect_name("expected property name after `.`")?;
                expr = Expr::Property {
                    object: Box::new(expr),
                    name,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(value) => Ok(Expr::Number(value)),
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::Identifier(value) => Ok(Expr::Identifier(value)),
            TokenKind::Keyword(Keyword::True) => Ok(Expr::Bool(true)),
            TokenKind::Keyword(Keyword::False) => Ok(Expr::Bool(false)),
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen, "expected `)` after expression")?;
                Ok(Expr::Group(Box::new(expr)))
            }
            TokenKind::LBracket => {
                let items = self.parse_expression_list(TokenKind::RBracket)?;
                Ok(Expr::List(items))
            }
            TokenKind::LBrace => Ok(Expr::Object(self.parse_object_fields_inner()?)),
            TokenKind::Keyword(keyword)
                if matches!(
                    keyword,
                    Keyword::Text
                        | Keyword::Input
                        | Keyword::Button
                        | Keyword::Api
                        | Keyword::Model
                        | Keyword::IdVerse
                        | Keyword::Print
                        | Keyword::Header
                        | Keyword::P
                        | Keyword::H1
                ) =>
            {
                Ok(Expr::Identifier(self.keyword_name(keyword).to_string()))
            }
            _ => Err(Diagnostic::new(
                "expected a value or expression here. You may have an unfinished assignment or a missing closing `)` or `]`.",
                token.line,
                token.column,
            )),
        }
    }

    fn parse_expression_list(&mut self, terminator: TokenKind) -> Result<Vec<Expr>, Diagnostic> {
        let mut items = Vec::new();
        if self.check(&terminator) {
            self.advance();
            return Ok(items);
        }
        loop {
            items.push(self.parse_expression()?);
            if self.matches(&TokenKind::Comma) {
                continue;
            }
            self.expect(terminator.clone(), "expected closing token")?;
            break;
        }
        Ok(items)
    }

    fn parse_object_fields(&mut self) -> Result<Vec<(String, Expr)>, Diagnostic> {
        self.expect(TokenKind::LBrace, "expected `{` to start object")?;
        self.parse_object_fields_inner()
    }

    fn parse_object_fields_inner(&mut self) -> Result<Vec<(String, Expr)>, Diagnostic> {
        let mut fields = Vec::new();
        self.skip_separators();
        if self.check(&TokenKind::RBrace) {
            self.advance();
            return Ok(fields);
        }
        loop {
            self.skip_separators();
            let key = self.expect_name("expected object key")?;
            self.expect(TokenKind::Colon, "expected `:` after object key")?;
            let value = self.parse_expression()?;
            fields.push((key, value));
            if self.matches(&TokenKind::Comma) {
                self.skip_separators();
                continue;
            }
            self.skip_separators();
            self.expect(TokenKind::RBrace, "expected `}` after object literal")?;
            break;
        }
        Ok(fields)
    }

    fn parse_call_argument(&mut self, name: &str) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LParen, format!("expected `(` after `{name}`"))?;
        let expr = self.parse_expression()?;
        self.expect(TokenKind::RParen, format!("expected `)` after `{name}` argument"))?;
        Ok(expr)
    }

    fn at_statement_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof)
    }

    fn skip_separators(&mut self) {
        while self.matches(&TokenKind::Newline) {}
    }

    fn synchronize_item(&mut self) {
        while !self.is_at_end() {
            if self.matches(&TokenKind::Newline) {
                self.skip_separators();
                break;
            }
            match self.peek().kind {
                TokenKind::Keyword(Keyword::Import)
                | TokenKind::Keyword(Keyword::Test)
                | TokenKind::Keyword(Keyword::Function)
                | TokenKind::Keyword(Keyword::Async)
                | TokenKind::Keyword(Keyword::App)
                | TokenKind::Keyword(Keyword::Web)
                | TokenKind::Keyword(Keyword::Api)
                | TokenKind::Keyword(Keyword::Db)
                | TokenKind::Keyword(Keyword::Table)
                | TokenKind::Keyword(Keyword::Model)
                | TokenKind::Keyword(Keyword::IdVerse) => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn synchronize_statement(&mut self) {
        while !self.is_at_end() {
            if self.matches(&TokenKind::Newline) {
                self.skip_separators();
                break;
            }
            match self.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::If)
                | TokenKind::Keyword(Keyword::Try)
                | TokenKind::Keyword(Keyword::For)
                | TokenKind::Keyword(Keyword::Return)
                | TokenKind::Keyword(Keyword::Print)
                | TokenKind::Keyword(Keyword::Insert)
                | TokenKind::Keyword(Keyword::Query) => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn synchronize_ui_node(&mut self) {
        while !self.is_at_end() {
            if self.matches(&TokenKind::Newline) {
                self.skip_separators();
                break;
            }
            match self.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Keyword(Keyword::Text)
                | TokenKind::Keyword(Keyword::Input)
                | TokenKind::Keyword(Keyword::Button)
                | TokenKind::Keyword(Keyword::Header)
                | TokenKind::Keyword(Keyword::P)
                | TokenKind::Keyword(Keyword::H1)
                | TokenKind::Keyword(Keyword::Scene) => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn synchronize_block_item(&mut self) {
        while !self.is_at_end() {
            if self.matches(&TokenKind::Newline) {
                self.skip_separators();
                break;
            }
            if self.check(&TokenKind::RBrace) {
                break;
            }
            self.advance();
        }
    }

    fn merge_diagnostics(&self, mut errors: Vec<Diagnostic>) -> Diagnostic {
        let mut primary = errors
            .drain(..1)
            .next()
            .unwrap_or_else(|| Diagnostic::new("parse error", 0, 0));
        for error in errors {
            primary = primary.with_related(error);
        }
        if !primary.related.is_empty() {
            let recovered = primary.related.len();
            primary = primary.with_note(format!(
                "parser recovered and found {recovered} additional issue(s)"
            ));
        }
        primary
    }

    fn expect_keyword(&mut self, keyword: Keyword, message: &str) -> Result<(), Diagnostic> {
        if self.matches_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn expect(&mut self, kind: TokenKind, message: impl Into<String>) -> Result<(), Diagnostic> {
        if self.check(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, Diagnostic> {
        match self.advance().kind.clone() {
            TokenKind::Identifier(value) => Ok(value),
            _ => Err(self.error_previous(message)),
        }
    }

    fn expect_string(&mut self, message: &str) -> Result<String, Diagnostic> {
        match self.advance().kind.clone() {
            TokenKind::String(value) => Ok(value),
            _ => Err(self.error_previous(message)),
        }
    }

    fn expect_name(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(value) => Ok(value),
            TokenKind::Keyword(keyword) => Ok(self.keyword_name(keyword).to_string()),
            _ => Err(Diagnostic::new(message, token.line, token.column)),
        }
    }

    fn matches_keyword(&mut self, keyword: Keyword) -> bool {
        if matches!(self.peek().kind, TokenKind::Keyword(found) if found == keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return matches!(kind, TokenKind::Eof);
        }
        &self.peek().kind == kind
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current.saturating_sub(1)]
    }

    fn error_here(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.peek();
        Diagnostic::new(
            self.friendly_message(message.into(), token),
            token.line,
            token.column,
        )
    }

    fn error_previous(&self, message: impl Into<String>) -> Diagnostic {
        let previous = self.previous();
        Diagnostic::new(
            self.friendly_message(message.into(), previous),
            previous.line,
            previous.column,
        )
    }

    fn friendly_message(&self, message: String, token: &Token) -> String {
        if message.contains("expected `}`") {
            return format!(
                "You may have missed a closing bracket `}}` near line {}.",
                token.line
            );
        }
        if message.contains("expected `)`") {
            return format!(
                "You may have missed a closing bracket `)` near line {}.",
                token.line
            );
        }
        if message.contains("expected `]`") {
            return format!(
                "You may have missed a closing bracket `]` near line {}.",
                token.line
            );
        }
        if message == "expected `=` after variable name" {
            return "A variable declaration needs `=` after the name. Example: `let total = 5`."
                .to_string();
        }
        if message == "expected `catch` after `try` block" {
            return "A `try` block must be followed by `catch err { ... }`.".to_string();
        }
        if message == "expected module name after `import`" {
            return "An import needs a module name. Example: `import std.ui` or `import \"support/math.egl\"`.".to_string();
        }
        message
    }

    fn keyword_name(&self, keyword: Keyword) -> &'static str {
        match keyword {
            Keyword::Import => "import",
            Keyword::Test => "test",
            Keyword::App => "app",
            Keyword::Screen => "screen",
            Keyword::Text => "text",
            Keyword::Input => "input",
            Keyword::Button => "button",
            Keyword::Api => "api",
            Keyword::Db => "db",
            Keyword::Model => "model",
            Keyword::IdVerse => "idverse",
            Keyword::Let => "let",
            Keyword::Function => "function",
            Keyword::Async => "async",
            Keyword::Await => "await",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::Try => "try",
            Keyword::Catch => "catch",
            Keyword::For => "for",
            Keyword::In => "in",
            Keyword::Return => "return",
            Keyword::Print => "print",
            Keyword::Connect => "connect",
            Keyword::Table => "table",
            Keyword::Insert => "insert",
            Keyword::Query => "query",
            Keyword::Where => "where",
            Keyword::Web => "web",
            Keyword::Page => "page",
            Keyword::Permissions => "permissions",
            Keyword::Type => "type",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::And => "and",
            Keyword::Or => "or",
            Keyword::Optional => "optional",
            Keyword::Header => "header",
            Keyword::P => "p",
            Keyword::H1 => "h1",
            Keyword::Scene => "scene",
        }
    }
}
