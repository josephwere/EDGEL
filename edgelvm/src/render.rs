use crate::ast::*;

pub fn render_preview_document(program: &Program) -> Option<String> {
    if let Some(app) = program.items.iter().find_map(|item| match item {
        Item::App(app) => Some(app),
        _ => None,
    }) {
        return Some(render_app_document(app));
    }

    if let Some(web) = program.items.iter().find_map(|item| match item {
        Item::Web(web) => Some(web),
        _ => None,
    }) {
        return Some(render_web_document(web));
    }

    None
}

pub fn program_summary(program: &Program) -> String {
    let mut counts = SummaryCounts::default();
    for item in &program.items {
        match item {
            Item::Import(_) => counts.imports += 1,
            Item::Statement(_) => counts.statements += 1,
            Item::Function(_) => counts.functions += 1,
            Item::Test(_) => counts.tests += 1,
            Item::App(app) => {
                counts.apps += 1;
                counts.screens += app.screens.len();
            }
            Item::Web(web) => {
                counts.web_apps += 1;
                counts.pages += web.pages.len();
                counts.apis += web.apis.len();
            }
            Item::Api(_) => counts.apis += 1,
            Item::Db(_) => counts.databases += 1,
            Item::Table(_) => counts.tables += 1,
            Item::Model(_) => counts.models += 1,
            Item::IdVerse(_) => counts.identities += 1,
        }
    }

    format!(
        "{} import(s), {} app(s), {} web project(s), {} screen(s), {} page(s), {} API(s), {} function(s), {} test(s), {} table(s), {} model(s), {} identity block(s), {} top-level statement(s)",
        counts.imports,
        counts.apps,
        counts.web_apps,
        counts.screens,
        counts.pages,
        counts.apis,
        counts.functions,
        counts.tests,
        counts.tables,
        counts.models,
        counts.identities,
        counts.statements
    )
}

#[derive(Default)]
struct SummaryCounts {
    imports: usize,
    statements: usize,
    functions: usize,
    tests: usize,
    apps: usize,
    web_apps: usize,
    screens: usize,
    pages: usize,
    apis: usize,
    databases: usize,
    tables: usize,
    models: usize,
    identities: usize,
}

fn render_app_document(app: &AppDecl) -> String {
    let initial = app
        .screens
        .first()
        .map(|screen| screen.name.clone())
        .unwrap_or_else(|| "Main".to_string());

    let screens = app
        .screens
        .iter()
        .map(|screen| {
            let content = screen
                .nodes
                .iter()
                .map(render_ui_node)
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r#"<section class="edgel-screen{}" data-screen="{}">
<div class="edgel-screen__inner">
{}
</div>
</section>"#,
                if screen.name == initial { " is-active" } else { "" },
                escape_html(&screen.name),
                content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let permissions = if app.permissions.is_empty() {
        String::new()
    } else {
        format!(
            r#"<aside class="edgel-permissions">Permissions: {}</aside>"#,
            escape_html(&app.permissions.join(", "))
        )
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
  <main class="edgel-shell">
    <div class="edgel-device">
      <header class="edgel-topbar">
        <div>
          <p class="edgel-kicker">GoldEdge Browser Preview</p>
          <h1>{}</h1>
        </div>
        {}
      </header>
      <div class="edgel-stage">
        {}
      </div>
    </div>
  </main>
  <script>{}</script>
</body>
</html>"#,
        escape_html(&app.name),
        preview_css(),
        escape_html(&app.name),
        permissions,
        screens,
        preview_runtime()
    )
}

fn render_web_document(web: &WebDecl) -> String {
    let page_html = web
        .pages
        .iter()
        .map(|page| {
            let content = page
                .nodes
                .iter()
                .map(render_ui_node)
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r#"<article class="edgel-page">
  <div class="edgel-page__route">{}</div>
  {}
</article>"#,
                escape_html(&page.route),
                content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let api_html = web
        .apis
        .iter()
        .map(|api| format!(r#"<li><code>{}</code></li>"#, escape_html(&api.route)))
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
  <main class="edgel-shell">
    <div class="edgel-web-shell">
      <p class="edgel-kicker">EDGEL Web Preview</p>
      <h1>{}</h1>
      <section class="edgel-web-grid">
        <div class="edgel-web-main">{}</div>
        <aside class="edgel-web-side">
          <h2>APIs</h2>
          <ul>{}</ul>
        </aside>
      </section>
    </div>
  </main>
</body>
</html>"#,
        escape_html(&web.name),
        preview_css(),
        escape_html(&web.name),
        page_html,
        api_html
    )
}

fn render_ui_node(node: &UiNode) -> String {
    match node {
        UiNode::Text(expr) => format!(r#"<p class="edgel-copy">{}</p>"#, escape_html(&expr_to_text(expr))),
        UiNode::Header(expr) => format!(r#"<h2 class="edgel-heading">{}</h2>"#, escape_html(&expr_to_text(expr))),
        UiNode::Paragraph(expr) => {
            format!(r#"<p class="edgel-copy is-soft">{}</p>"#, escape_html(&expr_to_text(expr)))
        }
        UiNode::Input {
            name,
            prompt,
            input_type,
        } => {
            let input_type = input_type.clone().unwrap_or_else(|| "text".to_string());
            let placeholder = prompt
                .as_ref()
                .map(expr_to_text)
                .unwrap_or_else(|| format!("Enter {}", name));
            format!(
                r#"<label class="edgel-input">
<span>{}</span>
<input id="input-{}" type="{}" placeholder="{}">
</label>"#,
                escape_html(name),
                escape_html(name),
                escape_html(&input_type),
                escape_html(&placeholder)
            )
        }
        UiNode::Button { label, actions } => format!(
            r#"<button class="edgel-button" onclick="{}">{}</button>"#,
            escape_attribute(&actions_to_js(actions)),
            escape_html(&expr_to_text(label))
        ),
        UiNode::Scene { commands } => {
            let items = commands
                .iter()
                .map(render_scene_command)
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<section class="edgel-scene">
  <div class="edgel-scene__glow"></div>
  <div class="edgel-scene__body">
    <h3>3D Scene Placeholder</h3>
    <ul>{}</ul>
  </div>
</section>"#,
                items
            )
        }
    }
}

fn render_scene_command(command: &SceneCommand) -> String {
    let args = command
        .args
        .iter()
        .map(expr_to_text)
        .collect::<Vec<_>>()
        .join(", ");
    let label = command.label.clone().unwrap_or_default();
    let children = if command.children.is_empty() {
        String::new()
    } else {
        format!(
            "<ul>{}</ul>",
            command
                .children
                .iter()
                .map(render_scene_command)
                .collect::<Vec<_>>()
                .join("")
        )
    };
    format!(
        "<li><strong>{}</strong> {} {}</li>{}",
        escape_html(&command.name),
        escape_html(&label),
        escape_html(&args),
        children
    )
}

fn actions_to_js(actions: &[Stmt]) -> String {
    let code = actions
        .iter()
        .map(stmt_to_js)
        .collect::<Vec<_>>()
        .join(";");
    if code.is_empty() {
        "edgelLog('No action attached.')".to_string()
    } else {
        code
    }
}

fn stmt_to_js(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Print { expr, .. } => format!("edgelLog({})", expr_to_js(expr)),
        Stmt::Expr { expr, .. } => match expr {
            Expr::Call { callee, args } => match callee.as_ref() {
                Expr::Identifier(name) if name == "alert" => format!(
                    "alert({})",
                    args.first()
                        .map(expr_to_js)
                        .unwrap_or_else(|| "\"\"".to_string())
                ),
                Expr::Identifier(name) if name == "navigate" => format!(
                    "edgelNavigate({})",
                    args.first()
                        .map(expr_to_js)
                        .unwrap_or_else(|| "\"Main\"".to_string())
                ),
                _ => format!("edgelLog({})", expr_to_js(expr)),
            },
            _ => format!("edgelLog({})", expr_to_js(expr)),
        },
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let then_code = then_branch
                .iter()
                .map(stmt_to_js)
                .collect::<Vec<_>>()
                .join(";");
            let else_code = else_branch
                .iter()
                .map(stmt_to_js)
                .collect::<Vec<_>>()
                .join(";");
            format!("if ({}) {{ {} }} else {{ {} }}", expr_to_js(condition), then_code, else_code)
        }
        _ => "edgelLog('Action not yet available in preview.')".to_string(),
    }
}

fn expr_to_js(expr: &Expr) -> String {
    match expr {
        Expr::Number(value) => value.to_string(),
        Expr::String(value) => format!("\"{}\"", escape_js(value)),
        Expr::Bool(value) => value.to_string(),
        Expr::Identifier(name) => format!("window.edgelState['{}'] ?? ''", escape_js(name)),
        Expr::List(items) => format!(
            "[{}]",
            items.iter().map(expr_to_js).collect::<Vec<_>>().join(", ")
        ),
        Expr::Object(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("{}: {}", key, expr_to_js(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Call { callee, args } => match callee.as_ref() {
            Expr::Identifier(name) if name == "fetch" => format!(
                "Promise.resolve({{ source: 'preview', url: {} }})",
                args.first()
                    .map(expr_to_js)
                    .unwrap_or_else(|| "\"\"".to_string())
            ),
            Expr::Identifier(name) if name == "now" => "new Date().toISOString()".to_string(),
            Expr::Property { object, name } => {
                if let Expr::Identifier(object_name) = object.as_ref() {
                    format!(
                        "(`NEUROEDGE {} via {}: ` + {})",
                        escape_js(name),
                        escape_js(object_name),
                        args.first()
                            .map(expr_to_js)
                            .unwrap_or_else(|| "\"\"".to_string())
                    )
                } else {
                    "\"unsupported-call\"".to_string()
                }
            }
            Expr::Identifier(name) => format!(
                "({}({}))",
                name,
                args.iter().map(expr_to_js).collect::<Vec<_>>().join(", ")
            ),
            _ => "\"unsupported-call\"".to_string(),
        },
        Expr::Property { object, name } => {
            if let Expr::Identifier(object_name) = object.as_ref() {
                if name == "value" {
                    format!(
                        "(document.getElementById('input-{}')?.value ?? '')",
                        escape_js(object_name)
                    )
                } else {
                    format!(
                        "(window.edgelState['{}']?.['{}'] ?? '')",
                        escape_js(object_name),
                        escape_js(name)
                    )
                }
            } else {
                "\"\"".to_string()
            }
        }
        Expr::Binary { left, op, right } => {
            let operator = match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Modulo => "%",
                BinaryOp::Equal => "===",
                BinaryOp::NotEqual => "!==",
                BinaryOp::Greater => ">",
                BinaryOp::GreaterEqual => ">=",
                BinaryOp::Less => "<",
                BinaryOp::LessEqual => "<=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
            };
            format!("({} {} {})", expr_to_js(left), operator, expr_to_js(right))
        }
        Expr::Unary { op, expr } => {
            let operator = match op {
                UnaryOp::Negate => "-",
                UnaryOp::Not => "!",
            };
            format!("({}{})", operator, expr_to_js(expr))
        }
        Expr::Await(expr) => expr_to_js(expr),
        Expr::Group(expr) => format!("({})", expr_to_js(expr)),
    }
}

fn expr_to_text(expr: &Expr) -> String {
    match expr {
        Expr::Number(value) => value.to_string(),
        Expr::String(value) => value.clone(),
        Expr::Bool(value) => value.to_string(),
        Expr::Identifier(value) => value.clone(),
        Expr::Property { object, name } => {
            if let Expr::Identifier(object_name) = object.as_ref() {
                format!("{object_name}.{name}")
            } else {
                "property".to_string()
            }
        }
        Expr::Binary { left, op, right } => {
            let operator = match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                BinaryOp::Modulo => "%",
                BinaryOp::Equal => "==",
                BinaryOp::NotEqual => "!=",
                BinaryOp::Greater => ">",
                BinaryOp::GreaterEqual => ">=",
                BinaryOp::Less => "<",
                BinaryOp::LessEqual => "<=",
                BinaryOp::And => "and",
                BinaryOp::Or => "or",
            };
            format!("{} {} {}", expr_to_text(left), operator, expr_to_text(right))
        }
        Expr::Unary { op, expr } => match op {
            UnaryOp::Negate => format!("-{}", expr_to_text(expr)),
            UnaryOp::Not => format!("not {}", expr_to_text(expr)),
        },
        Expr::Await(expr) => format!("await {}", expr_to_text(expr)),
        Expr::Group(expr) => expr_to_text(expr),
        Expr::Call { callee, args } => {
            let callee = expr_to_text(callee);
            let args = args.iter().map(expr_to_text).collect::<Vec<_>>().join(", ");
            format!("{callee}({args})")
        }
        Expr::List(items) => format!(
            "[{}]",
            items.iter().map(expr_to_text).collect::<Vec<_>>().join(", ")
        ),
        Expr::Object(entries) => format!(
            "{{ {} }}",
            entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", expr_to_text(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn preview_runtime() -> &'static str {
    r#"
window.edgelState = { user: { name: 'EDGEL User' } };
window.edgelLog = function (message) {
  console.log('[EDGEL]', message);
};
window.edgelNavigate = function (screenName) {
  const normalized = String(screenName).replace(/^"|"$/g, '');
  document.querySelectorAll('.edgel-screen').forEach((screen) => {
    screen.classList.toggle('is-active', screen.dataset.screen === normalized);
  });
};
"#
}

fn preview_css() -> &'static str {
    r#"
:root {
  color-scheme: light;
  --bg: radial-gradient(circle at top, #f9dca3 0%, #f8f4ed 45%, #f1eee8 100%);
  --card: rgba(255, 255, 255, 0.82);
  --ink: #1f2937;
  --muted: #5c6778;
  --gold: #b7791f;
  --gold-soft: #f8d58b;
  --border: rgba(31, 41, 55, 0.12);
  --shadow: 0 24px 60px rgba(99, 72, 26, 0.18);
}

* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  background: var(--bg);
  font-family: "IBM Plex Sans", "Trebuchet MS", sans-serif;
  color: var(--ink);
}
.edgel-shell {
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 32px 16px;
}
.edgel-device,
.edgel-web-shell {
  width: min(980px, 100%);
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 28px;
  box-shadow: var(--shadow);
  backdrop-filter: blur(16px);
  overflow: hidden;
}
.edgel-device { max-width: 420px; }
.edgel-topbar,
.edgel-web-shell {
  padding: 24px;
}
.edgel-kicker {
  margin: 0 0 8px;
  text-transform: uppercase;
  letter-spacing: 0.18em;
  font-size: 0.72rem;
  color: var(--gold);
}
h1, h2, h3, p { margin-top: 0; }
.edgel-stage {
  padding: 0 16px 16px;
}
.edgel-screen {
  display: none;
  padding: 16px;
}
.edgel-screen.is-active {
  display: block;
  animation: rise 280ms ease-out;
}
.edgel-screen__inner,
.edgel-page {
  background: rgba(255, 255, 255, 0.9);
  border-radius: 22px;
  border: 1px solid var(--border);
  padding: 18px;
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.8);
}
.edgel-heading {
  font-family: "Aptos Display", "Gill Sans", sans-serif;
  font-size: 1.45rem;
  margin-bottom: 10px;
}
.edgel-copy {
  color: var(--muted);
  line-height: 1.6;
}
.edgel-copy.is-soft { opacity: 0.82; }
.edgel-input {
  display: grid;
  gap: 8px;
  margin: 16px 0;
  font-size: 0.92rem;
}
.edgel-input input {
  width: 100%;
  padding: 14px 16px;
  border-radius: 16px;
  border: 1px solid rgba(183, 121, 31, 0.28);
  background: rgba(255,255,255,0.95);
}
.edgel-button {
  margin-top: 12px;
  width: 100%;
  border: 0;
  border-radius: 18px;
  padding: 14px 18px;
  background: linear-gradient(135deg, #1f2937 0%, #8f5b18 100%);
  color: white;
  font-weight: 700;
  cursor: pointer;
}
.edgel-button:hover { transform: translateY(-1px); }
.edgel-permissions {
  margin-top: 12px;
  color: var(--muted);
  font-size: 0.85rem;
}
.edgel-scene {
  position: relative;
  margin-top: 16px;
  min-height: 220px;
  border-radius: 24px;
  background: linear-gradient(160deg, #152238, #26476d 62%, #5076ab);
  overflow: hidden;
  color: white;
}
.edgel-scene__glow {
  position: absolute;
  inset: auto -40px -80px auto;
  width: 180px;
  height: 180px;
  background: radial-gradient(circle, rgba(255,215,138,0.9) 0%, rgba(255,215,138,0) 72%);
}
.edgel-scene__body {
  position: relative;
  padding: 20px;
}
.edgel-web-grid {
  display: grid;
  grid-template-columns: 1.7fr 1fr;
  gap: 18px;
}
.edgel-web-main,
.edgel-web-side {
  display: grid;
  gap: 16px;
}
.edgel-page__route {
  display: inline-block;
  margin-bottom: 14px;
  padding: 6px 10px;
  border-radius: 999px;
  background: rgba(183, 121, 31, 0.12);
  color: var(--gold);
  font-family: "IBM Plex Mono", monospace;
}
@media (max-width: 760px) {
  .edgel-web-grid { grid-template-columns: 1fr; }
}
@keyframes rise {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}
"#
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attribute(value: &str) -> String {
    escape_html(value).replace('\n', " ")
}

fn escape_js(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
