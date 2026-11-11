//! The approved report as a PDF (K1, K2), in the patient page's design: a
//! short letter from the clinic. Made on request from the [`Letter`] and
//! never stored. Built-in Helvetica keeps it free of embedded fonts; the
//! text is the letter's, word for word.

use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rect, Rgb, path::PaintMode,
};

use crate::letter::Letter;

const PAGE: (f32, f32) = (210.0, 297.0);
const MARGIN: f32 = 18.0;
const BOTTOM: f32 = 22.0;
/// The columns' left edges and widths, in mm: test, value, range, meaning.
const COLUMNS: [(f32, f32); 4] = [(18.0, 38.0), (58.0, 28.0), (88.0, 32.0), (122.0, 70.0)];

/// The letter as PDF bytes.
pub fn render(letter: &Letter) -> Result<Vec<u8>, printpdf::Error> {
    let (doc, page, layer) = PdfDocument::new("Your lab report", Mm(PAGE.0), Mm(PAGE.1), "Report");
    let fonts = Fonts {
        regular: doc.add_builtin_font(BuiltinFont::Helvetica)?,
        bold: doc.add_builtin_font(BuiltinFont::HelveticaBold)?,
    };
    let mut pen = Pen {
        layer: doc.get_page(page).get_layer(layer),
        y: PAGE.1 - MARGIN,
        fonts,
    };
    // The demo note, then the clinic's name across the top above a teal rule.
    pen.fill(MARGIN, pen.y - 8.0, PAGE.0 - MARGIN, pen.y, AMBER);
    pen.text(
        "Made-up demo: not a real patient",
        MARGIN + 3.0,
        pen.y - 5.5,
        9.0,
        true,
        INK,
    );
    pen.y -= 15.0;
    pen.text(&letter.clinic, MARGIN, pen.y, 13.0, true, INK);
    let label = "LAB RESULTS";
    pen.text(label, PAGE.0 - MARGIN - width(label, 8.0), pen.y, 8.0, false, MUTED);
    pen.y -= 4.0;
    pen.rule(TEAL, 1.2);
    pen.y -= 11.0;
    pen.text("Your lab report", MARGIN, pen.y, 18.0, true, INK);
    pen.y -= 7.5;
    let who = format!("{} · {} · {}", letter.patient, letter.report, letter.clinic);
    pen.paragraph(&who, MARGIN, PAGE.0 - 2.0 * MARGIN, 10.0, false, INK);
    pen.paragraph(&letter.made, MARGIN, PAGE.0 - 2.0 * MARGIN, 9.0, false, MUTED);
    pen.y -= 4.0;
    header(&mut pen);
    // A later page says whose report it carries on.
    let continued = format!("{} · {}, continued", letter.patient, letter.report);
    for row in &letter.rows {
        let cells = [
            wrap(&row.test, COLUMNS[0].1, 9.5),
            wrap(&row.value, COLUMNS[1].1, 9.5),
            wrap(&row.range, COLUMNS[2].1, 9.5),
            wrap(&row.explanation, COLUMNS[3].1, 9.5),
        ];
        let lines = cells.iter().map(Vec::len).max().unwrap_or(1).max(2);
        let height = lines as f32 * 4.6 + 3.5;
        if pen.y - height < BOTTOM {
            pen = pen.next_page(&doc, &continued);
            header(&mut pen);
        }
        let top = pen.y - 4.0;
        for (column, lines) in cells.iter().enumerate() {
            let bold = column == 0 || (column == 1 && row.outside);
            for (n, line) in lines.iter().enumerate() {
                pen.text(line, COLUMNS[column].0, top - n as f32 * 4.6, 9.5, bold, INK);
            }
        }
        let flag_y = top - cells[2].len() as f32 * 4.6;
        pen.text(
            row.flag,
            COLUMNS[2].0,
            flag_y,
            8.0,
            row.outside,
            if row.outside { WARN } else { MUTED },
        );
        pen.y -= height;
        pen.rule(RULE, 0.4);
    }
    pen.y -= 6.0;
    if pen.y < BOTTOM + 10.0 {
        pen = pen.next_page(&doc, &continued);
    }
    let close = format!(
        "Questions? Contact {}. This report explains your results; it doesn't diagnose anything.",
        letter.clinic
    );
    pen.paragraph(&close, MARGIN, PAGE.0 - 2.0 * MARGIN, 9.0, false, MUTED);
    doc.save_to_bytes()
}

/// The table's headings, as on a printed lab report.
fn header(pen: &mut Pen) {
    for (heading, (x, _)) in ["TEST", "YOUR VALUE", "THE LAB'S RANGE", "WHAT IT MEANS"]
        .iter()
        .zip(COLUMNS)
    {
        pen.text(heading, x, pen.y, 7.5, false, MUTED);
    }
    pen.y -= 2.5;
    pen.rule(RULE, 0.6);
}

const INK: (f32, f32, f32) = (0.15, 0.14, 0.12);
const MUTED: (f32, f32, f32) = (0.42, 0.40, 0.36);
const TEAL: (f32, f32, f32) = (0.06, 0.35, 0.36);
const AMBER: (f32, f32, f32) = (0.98, 0.94, 0.85);
const WARN: (f32, f32, f32) = (0.36, 0.22, 0.0);
const RULE: (f32, f32, f32) = (0.89, 0.87, 0.84);

struct Fonts {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
}

/// Where the next line goes, on which page.
struct Pen {
    layer: PdfLayerReference,
    /// From the bottom of the page, in mm.
    y: f32,
    fonts: Fonts,
}

impl Pen {
    /// A new page, headed by `heading`.
    fn next_page(self, doc: &printpdf::PdfDocumentReference, heading: &str) -> Self {
        let (page, layer) = doc.add_page(Mm(PAGE.0), Mm(PAGE.1), "Report");
        let pen = Self {
            layer: doc.get_page(page).get_layer(layer),
            y: PAGE.1 - MARGIN,
            fonts: self.fonts,
        };
        pen.text(heading, MARGIN, pen.y, 8.0, false, MUTED);
        Self { y: pen.y - 8.0, ..pen }
    }

    fn text(&self, text: &str, x: f32, y: f32, size: f32, bold: bool, color: (f32, f32, f32)) {
        self.layer.set_fill_color(rgb(color));
        let font = if bold { &self.fonts.bold } else { &self.fonts.regular };
        self.layer.use_text(text, size, Mm(x), Mm(y), font);
    }

    /// `text` wrapped to `width`, from the pen down.
    fn paragraph(&mut self, text: &str, x: f32, width: f32, size: f32, bold: bool, color: (f32, f32, f32)) {
        for line in wrap(text, width, size) {
            self.text(&line, x, self.y, size, bold, color);
            self.y -= size * 0.5;
        }
        self.y -= 1.5;
    }

    fn rule(&self, color: (f32, f32, f32), thickness: f32) {
        self.layer.set_outline_color(rgb(color));
        self.layer.set_outline_thickness(thickness);
        self.layer.add_line(Line {
            points: vec![
                (Point::new(Mm(MARGIN), Mm(self.y)), false),
                (Point::new(Mm(PAGE.0 - MARGIN), Mm(self.y)), false),
            ],
            is_closed: false,
        });
    }

    fn fill(&self, left: f32, bottom: f32, right: f32, top: f32, color: (f32, f32, f32)) {
        self.layer.set_fill_color(rgb(color));
        let mut rect = Rect::new(Mm(left), Mm(bottom), Mm(right), Mm(top));
        rect.mode = PaintMode::Fill;
        self.layer.add_rect(rect);
    }
}

fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb::new(r, g, b, None))
}

/// About how wide `text` is in Helvetica at `size` points, in mm.
fn width(text: &str, size: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | '\'' | '|' | '!' | ':' | ';' | ' ' => 0.28,
            'f' | 't' | 'r' | '(' | ')' | '-' => 0.36,
            'm' | 'w' | 'M' | 'W' => 0.85,
            c if c.is_ascii_uppercase() => 0.68,
            c if c.is_ascii_digit() => 0.56,
            _ => 0.53,
        })
        .sum();
    em * size * 0.3528
}

/// `text` broken into lines no wider than `max` mm, at word boundaries.
fn wrap(text: &str, max: f32, size: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if width(&candidate, size) > max && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}
