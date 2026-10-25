//! 検査済みの台帳から Markdown を作る。キーの順に並べ、日時や環境に依存させない。

use std::fmt::Write;

use super::{Decisions, Ledger, issue_url};

fn text(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' => out.push_str("<br>"),
            '\r' => {}
            '\\' | '|' | '[' | ']' | '`' | '*' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn link(label: &str, target: &str) -> String {
    let target = target
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('|', "%7C");
    format!("[{}]({target})", text(label))
}

fn list(out: &mut String, label: &str, values: &[String]) {
    if !values.is_empty() {
        let _ = writeln!(
            out,
            "- {label}: {}",
            values
                .iter()
                .map(|v| text(v))
                .collect::<Vec<_>>()
                .join("・")
        );
    }
}

fn optional(out: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        let _ = writeln!(out, "- {label}: {}", text(value));
    }
}

pub(super) fn render(ledger: &Ledger, decisions: &Decisions) -> String {
    let mut out = String::from(
        "<!-- formats/ledger.toml から cargo xtask codegen で生成。直接編集しない。 -->\n\n# 形式と権利の一覧\n\n[権利の台帳](../../formats/ledger.toml)から生成しています。ここにあるのは実装を置く方針であり、読み書きできる形式の一覧ではありません。変更の手順は [台帳の書き方](../../formats/README.md)、図面の要素の対応は [Drawing2D の対応表](drawing2d-mapping.md)を参照してください。\n\n- 権利: `public` は実装の公開に第三者の許諾が不要、`permission` は契約・会員・許諾が必要、`unknown` は条件が未確認です。\n- 置き場所: `core` は核、`plugin` は外部のプラグイン、`none` は実装しない方針です。\n- 台帳は判断した時点の記録です。URL の到達や権利の変更は、生成時に再確認していません。\n\n## 一覧\n\n| キー | 名前 | 系統 | 領域 | 権利 | 置き場所 |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for (id, f) in &ledger.formats {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | `{}` | `{}` |",
            link(id, &format!("#format-{id}")),
            text(&f.name),
            text(&f.family),
            f.domains
                .iter()
                .map(|v| text(v))
                .collect::<Vec<_>>()
                .join("・"),
            f.rights,
            f.place
        );
    }
    out.push_str("\n## 仕様の入手先\n");
    for (id, spec) in &ledger.specs {
        let _ = writeln!(
            out,
            "\n### spec-{id}\n\n- 名前: {}\n- 公開元: {}",
            text(&spec.title),
            text(&spec.publisher)
        );
        optional(&mut out, "版", spec.version.as_deref());
        let access = match spec.access.as_str() {
            "public" => "無償公開",
            "members" => "会員などに限定",
            "paid" => "有償",
            _ => &spec.access,
        };
        let _ = writeln!(
            out,
            "- 入手の条件: {access}\n- 入手先: {}\n- 確認日: {}",
            link(&spec.title, &spec.url),
            spec.checked
        );
        optional(&mut out, "補足", spec.note.as_deref());
    }
    out.push_str("\n## 形式ごとの根拠と記録\n");
    for (id, f) in &ledger.formats {
        let _ = writeln!(
            out,
            "\n### format-{id}\n\n- 名前: {}\n- 権利の根拠: {}",
            text(&f.name),
            text(&f.rights_note)
        );
        optional(&mut out, "プラグインの置き場所", f.repository.as_deref());
        let specs: Vec<_> = f
            .specs
            .iter()
            .map(|id| link(id, &format!("#spec-{id}")))
            .collect();
        let _ = writeln!(
            out,
            "- 仕様: {}",
            if specs.is_empty() {
                "登録なし".to_string()
            } else {
                specs.join("、")
            }
        );
        list(&mut out, "実装の根拠", &f.basis);
        for evidence in &f.evidence {
            let _ = writeln!(
                out,
                "- 権利の根拠にした文書: {}（確認日: {}）",
                link(&evidence.title, &evidence.url),
                evidence.checked
            );
            optional(&mut out, "文書の補足", evidence.note.as_deref());
        }
        for reference in &f.references {
            let _ = writeln!(
                out,
                "- 参照した実装: {}（{}）",
                link(&reference.name, &reference.url),
                text(&reference.license)
            );
        }
        list(&mut out, "照合するソフト", &f.verify);
        list(&mut out, "サンプルの出どころ（Git に入れない）", &f.samples);
        list(&mut out, "既知の crate", &f.known_crates);
        for number in &f.decisions {
            if let Some(file) = decisions.get(number) {
                let _ = writeln!(
                    out,
                    "- 記録: {}",
                    link(&format!("決定 {number}"), &format!("../decisions/{file}"))
                );
            }
        }
        if !f.legacy_decisions.is_empty() {
            let _ = writeln!(
                out,
                "- 旧版の記録: {}（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）",
                f.legacy_decisions
                    .iter()
                    .map(|n| format!("旧版の決定 {n}"))
                    .collect::<Vec<_>>()
                    .join("、")
            );
        }
        for issue in &f.issues {
            if let Some(url) = issue_url(issue) {
                let _ = writeln!(out, "- issue: {}", link(issue, &url));
            }
        }
        for doc in &f.docs {
            let _ = writeln!(out, "- 形式の文書: {}", link(doc, &format!("../../{doc}")));
        }
        optional(&mut out, "補足", f.notes.as_deref());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_text_without_breaking_tables_or_links() {
        assert_eq!(
            text("a|[b] <c>\n& `d`"),
            "a\\|\\[b\\] &lt;c&gt;<br>&amp; \\`d\\`"
        );
        assert_eq!(
            link("[a]", "https://example.com/(a)|b"),
            "[\\[a\\]](https://example.com/%28a%29%7Cb)"
        );
    }
}
