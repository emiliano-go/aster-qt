use rune::{Cx, El, Env, Runtime};

use crate::{render, MailStyle};

struct State {
    html: String,
    style: MailStyle,
}

fn style() -> MailStyle {
    use rune::Color;
    MailStyle {
        text: Color::WHITE,
        muted: Color::WHITE.alpha(0.7),
        link: Color::WHITE.alpha(0.85),
        code_bg: Color::WHITE.alpha(0.08),
        quote_border: Color::WHITE.alpha(0.2),
        rule: Color::WHITE.alpha(0.15),
        surface: Color::WHITE.alpha(0.05),
        body_size: 14.0,
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let open: fn(&mut State, &str) = |_, _| {};
    render(&s.html, &s.style, None, 2.0, open)
}

#[test]
fn synthetic_html_renders() {
    let state = State {
        html: "<h2>Hi</h2><p>Hello <b>world</b> <a href=\"https://x.test\">link</a></p><ul><li>one</li></ul>".into(),
        style: style(),
    };
    let mut rt = Runtime::new(
        state,
        view,
        (800.0, 600.0),
        Env {
            system_dark: true,
            scale: 2.0,
        },
    );
    rt.frame(0.0);
}

#[test]
fn real_fixture_renders_when_present() {
    let Ok(path) = std::env::var("ASTER_MAILHTML_FIXTURE") else {
        return;
    };
    let Ok(html) = std::fs::read_to_string(path) else {
        return;
    };
    let state = State {
        html,
        style: style(),
    };
    let mut rt = Runtime::new(
        state,
        view,
        (900.0, 700.0),
        Env {
            system_dark: true,
            scale: 2.0,
        },
    );
    rt.frame(0.0);
}
