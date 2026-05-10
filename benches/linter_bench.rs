use criterion::{black_box, criterion_group, criterion_main, Criterion};
use djlintr::{config::Config, format, lint};

fn generate_large_template() -> String {
    let mut template = String::from("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <title>Benchmark</title>\n</head>\n<body>\n");
    for i in 0..1000 {
        template.push_str(&format!(
            "    <div id=\"item-{}\" class=\"container\">\n",
            i
        ));
        template.push_str("        <p>This is item {{ i }}</p>\n");
        template.push_str("        {% if i % 2 == 0 %}\n");
        template.push_str("            <span>Even</span>\n");
        template.push_str("        {% else %}\n");
        template.push_str("            <span>Odd</span>\n");
        template.push_str("        {% endif %}\n");
        template.push_str("        <img src=\"/static/img.png\" alt=\"image\">\n");
        template.push_str("    </div>\n");
    }
    template.push_str("</body>\n</html>");
    template
}

fn bench_linter(c: &mut Criterion) {
    let template = generate_large_template();
    let config = Config::default();

    c.bench_function("lint_large_template", |b| {
        b.iter(|| lint(black_box(&config), black_box(&template)))
    });
}

fn bench_formatter(c: &mut Criterion) {
    let template = generate_large_template();
    let config = Config::default();

    c.bench_function("format_large_template", |b| {
        b.iter(|| format(black_box(&config), black_box(&template)))
    });
}

criterion_group!(benches, bench_linter, bench_formatter);
criterion_main!(benches);
