use crate::ad_logger::AdPlayLogger;
use chrono::Local;
use printpdf::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Result of generating reports for a single ad.
#[derive(Debug)]
pub struct ReportResult {
    pub ad_name: String,
    pub csv_path: PathBuf,
    pub pdf_path: PathBuf,
}

/// Result of generating a multi-ad matrix report.
#[derive(Debug)]
pub struct MultiReportResult {
    pub path: PathBuf,
    pub format: ReportFormat,
}

/// Report output format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportFormat {
    Csv,
    Pdf,
}

impl ReportFormat {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            Self::Csv => "csv",
            Self::Pdf => "pdf",
        }
    }
}

/// Generates verified-play reports from ad play data.
pub struct AdReportGenerator<'a> {
    logger: &'a AdPlayLogger,
}

/// Hourly play entry for reports.
#[derive(Debug, Clone)]
struct HourlyEntry {
    date_iso: String,
    hour: u8,
    plays: usize,
}

/// Daily play entry for reports.
#[derive(Debug, Clone)]
struct DailyEntry {
    date_iso: String,
    total: usize,
}

impl<'a> AdReportGenerator<'a> {
    pub fn new(logger: &'a AdPlayLogger) -> Self {
        Self { logger }
    }

    /// Generate CSV and PDF reports for all ads with plays in the given period.
    /// Returns a list of report results (one per ad).
    pub fn generate_report(
        &self,
        start: &str,
        end: &str,
        company_name: Option<&str>,
        output_dir: &Path,
    ) -> Vec<ReportResult> {
        let hourly = self.logger.get_hourly_confirmed_stats(start, end);
        let daily = self.logger.get_daily_confirmed_stats(start, end);

        // Collect all ad names that have plays
        let mut ad_names: Vec<String> = Vec::new();
        for ads in hourly.values() {
            for name in ads.keys() {
                if !ad_names.contains(name) {
                    ad_names.push(name.clone());
                }
            }
        }
        ad_names.sort();

        let mut results = Vec::new();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

        for ad_name in &ad_names {
            let hourly_entries = self.extract_hourly(ad_name, &hourly);
            let daily_entries = self.extract_daily(ad_name, &daily);
            let total_plays: usize = daily_entries.iter().map(|d| d.total).sum();

            if total_plays == 0 {
                continue;
            }

            let safe_name = sanitize_filename(ad_name);
            let csv_path = output_dir.join(format!("REPORT_{}_{}.csv", safe_name, timestamp));
            let pdf_path = output_dir.join(format!("REPORT_{}_{}.pdf", safe_name, timestamp));

            let csv_content = self.build_csv(ad_name, start, end, &hourly_entries, &daily_entries, total_plays);
            let _ = std::fs::write(&csv_path, csv_content);

            let pdf_bytes = self.build_pdf(ad_name, start, end, company_name, &hourly_entries, &daily_entries, total_plays);
            let _ = std::fs::write(&pdf_path, pdf_bytes);

            results.push(ReportResult {
                ad_name: ad_name.clone(),
                csv_path,
                pdf_path,
            });
        }

        results
    }

    /// Generate a single-ad report (CSV + PDF). Returns None if no plays found.
    pub fn generate_single_report(
        &self,
        ad_name: &str,
        start: &str,
        end: &str,
        company_name: Option<&str>,
        output_dir: &Path,
    ) -> Option<ReportResult> {
        let hourly = self.logger.get_hourly_confirmed_stats(start, end);
        let daily = self.logger.get_daily_confirmed_stats(start, end);

        let hourly_entries = self.extract_hourly(ad_name, &hourly);
        let daily_entries = self.extract_daily(ad_name, &daily);
        let total_plays: usize = daily_entries.iter().map(|d| d.total).sum();

        if total_plays == 0 {
            return None;
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let safe_name = sanitize_filename(ad_name);
        let csv_path = output_dir.join(format!("REPORT_{}_{}.csv", safe_name, timestamp));
        let pdf_path = output_dir.join(format!("REPORT_{}_{}.pdf", safe_name, timestamp));

        let csv_content = self.build_csv(ad_name, start, end, &hourly_entries, &daily_entries, total_plays);
        let _ = std::fs::write(&csv_path, csv_content);

        let pdf_bytes = self.build_pdf(ad_name, start, end, company_name, &hourly_entries, &daily_entries, total_plays);
        let _ = std::fs::write(&pdf_path, pdf_bytes);

        Some(ReportResult {
            ad_name: ad_name.to_string(),
            csv_path,
            pdf_path,
        })
    }

    /// Generate a multi-ad matrix report.
    pub fn generate_multi_ad_report(
        &self,
        ad_names: &[String],
        start: &str,
        end: &str,
        output_file: &Path,
        format: ReportFormat,
    ) -> Option<MultiReportResult> {
        let daily = self.logger.get_daily_confirmed_stats(start, end);

        // Collect all dates sorted
        let mut dates: Vec<String> = daily.keys().cloned().collect();
        dates.sort();

        if dates.is_empty() {
            return None;
        }

        // Filter to requested ad names (or all if empty)
        let names: Vec<String> = if ad_names.is_empty() {
            let mut all: Vec<String> = Vec::new();
            for ads in daily.values() {
                for name in ads.keys() {
                    if !all.contains(name) {
                        all.push(name.clone());
                    }
                }
            }
            all.sort();
            all
        } else {
            ad_names.to_vec()
        };

        if names.is_empty() {
            return None;
        }

        match format {
            ReportFormat::Csv => {
                let content = self.build_multi_csv(&names, &dates, &daily, start, end);
                let _ = std::fs::write(output_file, content);
            }
            ReportFormat::Pdf => {
                let bytes = self.build_multi_pdf(&names, &dates, &daily, start, end);
                let _ = std::fs::write(output_file, bytes);
            }
        }

        Some(MultiReportResult {
            path: output_file.to_path_buf(),
            format,
        })
    }

    // --- CSV builders ---

    fn build_csv(
        &self,
        ad_name: &str,
        start: &str,
        end: &str,
        hourly: &[HourlyEntry],
        daily: &[DailyEntry],
        total_plays: usize,
    ) -> String {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let hours_with_airplay = hourly.len();
        let days_with_airplay = daily.len();

        let mut out = String::new();
        out.push_str("VERIFIED Advertiser Report\n\n");
        out.push_str(&format!("Ad Name: {}\n", ad_name));
        out.push_str(&format!("Report Period: {} to {}\n", start, end));
        out.push_str(&format!("Generated: {}\n", now));
        out.push_str(&format!("Total Confirmed Plays: {}\n", total_plays));
        out.push_str(&format!("Hours with Airplay: {}\n", hours_with_airplay));
        out.push_str(&format!("Days with Airplay: {}\n", days_with_airplay));
        out.push_str("\nHOURLY BREAKDOWN\n");
        out.push_str("Date,Hour,Plays\n");

        for entry in hourly {
            out.push_str(&format!("{},{:02}:00,{}\n", entry.date_iso, entry.hour, entry.plays));
        }

        out.push_str("\nDAILY SUMMARY\n");
        out.push_str("Date,Total Plays\n");

        for entry in daily {
            out.push_str(&format!("{},{}\n", entry.date_iso, entry.total));
        }

        out.push_str(&format!("\nGRAND TOTAL,{}\n", total_plays));
        out
    }

    fn build_multi_csv(
        &self,
        names: &[String],
        dates: &[String],
        daily: &std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
        start: &str,
        end: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str(&format!("Multi-Ad Report: {} to {}\n\n", start, end));

        // Header: Date, Ad1, Ad2, ...
        out.push_str("Date");
        for name in names {
            out.push_str(&format!(",{}", name));
        }
        out.push('\n');

        // Rows
        let mut totals: Vec<usize> = vec![0; names.len()];
        for date in dates {
            out.push_str(date);
            for (i, name) in names.iter().enumerate() {
                let count = daily
                    .get(date)
                    .and_then(|ads| ads.get(name))
                    .copied()
                    .unwrap_or(0);
                out.push_str(&format!(",{}", count));
                totals[i] += count;
            }
            out.push('\n');
        }

        // Totals row
        out.push_str("TOTAL");
        for t in &totals {
            out.push_str(&format!(",{}", t));
        }
        out.push('\n');
        out
    }

    // --- PDF builders ---

    fn build_pdf(
        &self,
        ad_name: &str,
        start: &str,
        end: &str,
        company_name: Option<&str>,
        hourly: &[HourlyEntry],
        daily: &[DailyEntry],
        total_plays: usize,
    ) -> Vec<u8> {
        let mut doc = PdfDocument::new("Ad Report");
        let hours_with_airplay = hourly.len();
        let days_with_airplay = daily.len();
        let avg_per_day = if days_with_airplay > 0 {
            total_plays as f64 / days_with_airplay as f64
        } else {
            0.0
        };
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let black = rgb_black();
        let white = rgb_white();
        let hdr_bg = rgb_header_bg();
        let alt = rgb_alt_row();
        let gray = rgb_gray();

        let mut ops: Vec<Op> = Vec::new();
        let mut y = Mm(277.0);

        // Title
        let title = match company_name {
            Some(c) => format!("VERIFIED Advertiser Report — {}", c),
            None => "VERIFIED Advertiser Report".to_string(),
        };
        pdf_text(&mut ops, &title, Mm(20.0), y, BuiltinFont::HelveticaBold, Pt(16.0), &black);
        y = y - Mm(10.0);

        // Report info
        pdf_text(&mut ops, &format!("Ad Name: {}", ad_name), Mm(20.0), y, BuiltinFont::Helvetica, Pt(10.0), &black);
        y = y - Mm(5.0);
        pdf_text(&mut ops, &format!("Report Period: {} to {}", start, end), Mm(20.0), y, BuiltinFont::Helvetica, Pt(10.0), &black);
        y = y - Mm(5.0);
        pdf_text(&mut ops, &format!("Generated: {}", now), Mm(20.0), y, BuiltinFont::Helvetica, Pt(10.0), &black);
        y = y - Mm(8.0);

        // Summary box
        let box_top = y;
        let box_bottom = y - Mm(22.0);
        let box_fill = Rgb { r: 0.95, g: 0.95, b: 0.97, icc_profile: None };
        let box_stroke = Rgb { r: 0.7, g: 0.7, b: 0.7, icc_profile: None };
        pdf_rect_fill(&mut ops, Mm(20.0), box_bottom, Mm(170.0), box_top, &box_fill);
        pdf_rect_stroke(&mut ops, Mm(20.0), box_bottom, Mm(170.0), box_top, &box_stroke);

        y = y - Mm(5.0);
        pdf_text(&mut ops, &format!("Total Confirmed Plays: {}", total_plays), Mm(25.0), y, BuiltinFont::HelveticaBold, Pt(10.0), &black);
        y = y - Mm(5.0);
        pdf_text(&mut ops, &format!("Hours with Airplay: {}    Days with Airplay: {}    Avg/Day: {:.1}", hours_with_airplay, days_with_airplay, avg_per_day), Mm(25.0), y, BuiltinFont::Helvetica, Pt(9.0), &black);
        y = box_bottom - Mm(8.0);

        // Hourly breakdown table
        pdf_text(&mut ops, "HOURLY BREAKDOWN", Mm(20.0), y, BuiltinFont::HelveticaBold, Pt(11.0), &black);
        y = y - Mm(6.0);

        let row_h = Mm(5.0);

        pdf_rect_fill(&mut ops, Mm(20.0), y - row_h, Mm(170.0), y, &hdr_bg);
        pdf_text(&mut ops, "Date", Mm(22.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(8.0), &white);
        pdf_text(&mut ops, "Hour", Mm(80.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(8.0), &white);
        pdf_text(&mut ops, "Plays", Mm(130.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(8.0), &white);
        y = y - row_h;

        for (i, entry) in hourly.iter().enumerate() {
            if y < Mm(30.0) {
                doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
                ops = Vec::new();
                y = Mm(277.0);
            }

            if i % 2 == 0 {
                pdf_rect_fill(&mut ops, Mm(20.0), y - row_h, Mm(170.0), y, &alt);
            }
            pdf_text(&mut ops, &entry.date_iso, Mm(22.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(8.0), &black);
            pdf_text(&mut ops, &format!("{:02}:00", entry.hour), Mm(80.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(8.0), &black);
            pdf_text(&mut ops, &entry.plays.to_string(), Mm(130.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(8.0), &black);
            y = y - row_h;
        }

        y = y - Mm(8.0);

        // Daily summary table
        if y < Mm(50.0) {
            doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
            ops = Vec::new();
            y = Mm(277.0);
        }

        pdf_text(&mut ops, "DAILY SUMMARY", Mm(20.0), y, BuiltinFont::HelveticaBold, Pt(11.0), &black);
        y = y - Mm(6.0);

        pdf_rect_fill(&mut ops, Mm(20.0), y - row_h, Mm(170.0), y, &hdr_bg);
        pdf_text(&mut ops, "Date", Mm(22.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(8.0), &white);
        pdf_text(&mut ops, "Total Plays", Mm(130.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(8.0), &white);
        y = y - row_h;

        for (i, entry) in daily.iter().enumerate() {
            if y < Mm(30.0) {
                doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
                ops = Vec::new();
                y = Mm(277.0);
            }

            if i % 2 == 0 {
                pdf_rect_fill(&mut ops, Mm(20.0), y - row_h, Mm(170.0), y, &alt);
            }
            pdf_text(&mut ops, &entry.date_iso, Mm(22.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(8.0), &black);
            pdf_text(&mut ops, &entry.total.to_string(), Mm(130.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(8.0), &black);
            y = y - row_h;
        }

        // Grand total row
        y = y - Mm(2.0);
        pdf_text(&mut ops, &format!("GRAND TOTAL: {}", total_plays), Mm(22.0), y, BuiltinFont::HelveticaBold, Pt(10.0), &black);

        // Footer
        pdf_text(&mut ops, "signalFlow — Radio Automation Engine", Mm(20.0), Mm(10.0), BuiltinFont::Helvetica, Pt(7.0), &gray);

        doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));

        let mut warnings = Vec::new();
        doc.save(&PdfSaveOptions::default(), &mut warnings)
    }

    fn build_multi_pdf(
        &self,
        names: &[String],
        dates: &[String],
        daily: &std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
        start: &str,
        end: &str,
    ) -> Vec<u8> {
        let mut doc = PdfDocument::new("Multi-Ad Report");
        let black = rgb_black();
        let white = rgb_white();
        let hdr_bg = rgb_header_bg();
        let alt = rgb_alt_row();
        let gray = rgb_gray();

        let mut ops: Vec<Op> = Vec::new();
        let mut y = Mm(277.0);

        // Title
        pdf_text(&mut ops, &format!("Multi-Ad Report: {} to {}", start, end), Mm(20.0), y, BuiltinFont::HelveticaBold, Pt(14.0), &black);
        y = y - Mm(10.0);

        // Calculate column widths
        let table_left = Mm(20.0);
        let table_right = Mm(190.0);
        let remaining: f32 = 190.0 - 20.0 - 30.0;
        let col_w = if names.is_empty() { 30.0_f32 } else { remaining / names.len() as f32 };
        let row_h = Mm(5.0);

        // Header row
        pdf_rect_fill(&mut ops, table_left, y - row_h, table_right, y, &hdr_bg);
        pdf_text(&mut ops, "Date", Mm(22.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(7.0), &white);

        for (i, name) in names.iter().enumerate() {
            let x = Mm(50.0 + i as f32 * col_w);
            let display = if name.len() > 12 { &name[..12] } else { name };
            pdf_text(&mut ops, display, x, y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(7.0), &white);
        }
        y = y - row_h;

        let mut totals: Vec<usize> = vec![0; names.len()];

        for (row_idx, date) in dates.iter().enumerate() {
            if y < Mm(30.0) {
                doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
                ops = Vec::new();
                y = Mm(277.0);
            }

            if row_idx % 2 == 0 {
                pdf_rect_fill(&mut ops, table_left, y - row_h, table_right, y, &alt);
            }
            pdf_text(&mut ops, date, Mm(22.0), y - Mm(3.5), BuiltinFont::Helvetica, Pt(7.0), &black);

            for (i, name) in names.iter().enumerate() {
                let count = daily.get(date).and_then(|ads| ads.get(name)).copied().unwrap_or(0);
                totals[i] += count;
                let x = Mm(50.0 + i as f32 * col_w);
                pdf_text(&mut ops, &count.to_string(), x, y - Mm(3.5), BuiltinFont::Helvetica, Pt(7.0), &black);
            }
            y = y - row_h;
        }

        // Totals row
        y = y - Mm(1.0);
        let totals_bg = Rgb { r: 0.85, g: 0.85, b: 0.9, icc_profile: None };
        pdf_rect_fill(&mut ops, table_left, y - row_h, table_right, y, &totals_bg);
        pdf_text(&mut ops, "TOTAL", Mm(22.0), y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(7.0), &black);
        for (i, t) in totals.iter().enumerate() {
            let x = Mm(50.0 + i as f32 * col_w);
            pdf_text(&mut ops, &t.to_string(), x, y - Mm(3.5), BuiltinFont::HelveticaBold, Pt(7.0), &black);
        }

        // Footer
        pdf_text(&mut ops, "signalFlow — Radio Automation Engine", Mm(20.0), Mm(10.0), BuiltinFont::Helvetica, Pt(7.0), &gray);

        doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));

        let mut warnings = Vec::new();
        doc.save(&PdfSaveOptions::default(), &mut warnings)
    }

    // --- Data extraction helpers ---

    fn extract_hourly(
        &self,
        ad_name: &str,
        hourly: &std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
    ) -> Vec<HourlyEntry> {
        // hourly keys are "YYYY-MM-DD_HH"
        let mut entries: BTreeMap<String, HourlyEntry> = BTreeMap::new();

        for (key, ads) in hourly {
            if let Some(&count) = ads.get(ad_name) {
                if count > 0 {
                    // Parse "YYYY-MM-DD_HH"
                    if let Some((date, hour_str)) = key.rsplit_once('_') {
                        let hour = hour_str.parse::<u8>().unwrap_or(0);
                        entries.insert(key.clone(), HourlyEntry {
                            date_iso: date.to_string(),
                            hour,
                            plays: count,
                        });
                    }
                }
            }
        }

        entries.into_values().collect()
    }

    fn extract_daily(
        &self,
        ad_name: &str,
        daily: &std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
    ) -> Vec<DailyEntry> {
        let mut entries: BTreeMap<String, DailyEntry> = BTreeMap::new();

        for (date, ads) in daily {
            if let Some(&count) = ads.get(ad_name) {
                if count > 0 {
                    entries.insert(date.clone(), DailyEntry {
                        date_iso: date.clone(),
                        total: count,
                    });
                }
            }
        }

        entries.into_values().collect()
    }
}

// --- PDF helper functions ---

fn rgb_black() -> Rgb { Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None } }
fn rgb_white() -> Rgb { Rgb { r: 1.0, g: 1.0, b: 1.0, icc_profile: None } }
fn rgb_header_bg() -> Rgb { Rgb { r: 0.2, g: 0.2, b: 0.3, icc_profile: None } }
fn rgb_alt_row() -> Rgb { Rgb { r: 0.95, g: 0.95, b: 0.95, icc_profile: None } }
fn rgb_gray() -> Rgb { Rgb { r: 0.5, g: 0.5, b: 0.5, icc_profile: None } }

fn pdf_text(ops: &mut Vec<Op>, text: &str, x: Mm, y: Mm, font: BuiltinFont, size: Pt, color: &Rgb) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor { pos: Point::new(x, y) });
    ops.push(Op::SetFont { font: PdfFontHandle::Builtin(font), size });
    ops.push(Op::SetFillColor { col: Color::Rgb(color.clone()) });
    ops.push(Op::ShowText { items: vec![TextItem::Text(text.to_string())] });
    ops.push(Op::EndTextSection);
}

fn pdf_rect_fill(ops: &mut Vec<Op>, x1: Mm, y1: Mm, x2: Mm, y2: Mm, color: &Rgb) {
    ops.push(Op::SetFillColor { col: Color::Rgb(color.clone()) });
    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint { p: Point::new(x1, y1), bezier: false },
                    LinePoint { p: Point::new(x2, y1), bezier: false },
                    LinePoint { p: Point::new(x2, y2), bezier: false },
                    LinePoint { p: Point::new(x1, y2), bezier: false },
                ],
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        },
    });
}

fn pdf_rect_stroke(ops: &mut Vec<Op>, x1: Mm, y1: Mm, x2: Mm, y2: Mm, color: &Rgb) {
    ops.push(Op::SetOutlineColor { col: Color::Rgb(color.clone()) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint { p: Point::new(x1, y1), bezier: false },
                    LinePoint { p: Point::new(x2, y1), bezier: false },
                    LinePoint { p: Point::new(x2, y2), bezier: false },
                    LinePoint { p: Point::new(x1, y2), bezier: false },
                ],
            }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::NonZero,
        },
    });
}

/// Sanitize a string for use as a filename.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad_logger::AdPlayLogger;

    fn temp_logger() -> (AdPlayLogger, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let logger = AdPlayLogger::new(dir.path());
        (logger, dir)
    }

    fn seed_data(logger: &AdPlayLogger) {
        // Ad A: 3 plays on 01-15-26, 2 plays on 01-16-26
        logger.log_play_at("Ad Alpha", "01-15-26", 9);
        logger.log_play_at("Ad Alpha", "01-15-26", 10);
        logger.log_play_at("Ad Alpha", "01-15-26", 14);
        logger.log_play_at("Ad Alpha", "01-16-26", 9);
        logger.log_play_at("Ad Alpha", "01-16-26", 15);

        // Ad B: 2 plays on 01-15-26
        logger.log_play_at("Ad Beta", "01-15-26", 9);
        logger.log_play_at("Ad Beta", "01-15-26", 11);
    }

    #[test]
    fn generate_report_creates_csv_and_pdf_files() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        assert_eq!(results.len(), 2); // Ad Alpha and Ad Beta
        for r in &results {
            assert!(r.csv_path.exists(), "CSV not created for {}", r.ad_name);
            assert!(r.pdf_path.exists(), "PDF not created for {}", r.ad_name);
        }
    }

    #[test]
    fn generate_report_with_company_name() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", Some("ACME Corp"), &output);
        assert!(!results.is_empty());

        // Check CSV contains company-less header (company is PDF-only in title)
        let csv = std::fs::read_to_string(&results[0].csv_path).unwrap();
        assert!(csv.contains("VERIFIED Advertiser Report"));
    }

    #[test]
    fn csv_contains_hourly_and_daily_sections() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        let alpha = results.iter().find(|r| r.ad_name == "Ad Alpha").unwrap();
        let csv = std::fs::read_to_string(&alpha.csv_path).unwrap();

        assert!(csv.contains("HOURLY BREAKDOWN"));
        assert!(csv.contains("DAILY SUMMARY"));
        assert!(csv.contains("GRAND TOTAL,5"));
        assert!(csv.contains("Ad Name: Ad Alpha"));
        assert!(csv.contains("Total Confirmed Plays: 5"));
        assert!(csv.contains("Date,Hour,Plays"));
        assert!(csv.contains("Date,Total Plays"));
    }

    #[test]
    fn csv_hourly_data_is_sorted() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        let alpha = results.iter().find(|r| r.ad_name == "Ad Alpha").unwrap();
        let csv = std::fs::read_to_string(&alpha.csv_path).unwrap();

        // Extract hourly lines
        let hourly_section: Vec<&str> = csv
            .lines()
            .skip_while(|l| !l.starts_with("Date,Hour,Plays"))
            .skip(1)
            .take_while(|l| !l.is_empty())
            .collect();

        assert!(!hourly_section.is_empty());
        // Verify sorted (each line should be >= previous)
        for i in 1..hourly_section.len() {
            assert!(hourly_section[i] >= hourly_section[i - 1], "Hourly data not sorted");
        }
    }

    #[test]
    fn generate_report_returns_empty_for_no_data() {
        let (logger, dir) = temp_logger();
        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        assert!(results.is_empty());
    }

    #[test]
    fn generate_single_report_returns_none_for_unknown_ad() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let result = reporter.generate_single_report("Nonexistent", "01-10-26", "01-20-26", None, &output);
        assert!(result.is_none());
    }

    #[test]
    fn generate_single_report_creates_files() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let result = reporter.generate_single_report("Ad Alpha", "01-10-26", "01-20-26", None, &output);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.ad_name, "Ad Alpha");
        assert!(r.csv_path.exists());
        assert!(r.pdf_path.exists());
    }

    #[test]
    fn multi_ad_csv_report_has_matrix_format() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output_file = dir.path().join("multi.csv");

        let result = reporter.generate_multi_ad_report(
            &[],
            "01-10-26", "01-20-26",
            &output_file,
            ReportFormat::Csv,
        );
        assert!(result.is_some());

        let csv = std::fs::read_to_string(&output_file).unwrap();
        assert!(csv.contains("Date,Ad Alpha,Ad Beta") || csv.contains("Date,Ad Beta,Ad Alpha"));
        assert!(csv.contains("TOTAL"));
    }

    #[test]
    fn multi_ad_pdf_report_creates_file() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output_file = dir.path().join("multi.pdf");

        let result = reporter.generate_multi_ad_report(
            &["Ad Alpha".into(), "Ad Beta".into()],
            "01-10-26", "01-20-26",
            &output_file,
            ReportFormat::Pdf,
        );
        assert!(result.is_some());
        assert!(output_file.exists());
        let size = std::fs::metadata(&output_file).unwrap().len();
        assert!(size > 100, "PDF too small: {} bytes", size);
    }

    #[test]
    fn multi_ad_report_returns_none_for_no_data() {
        let (logger, dir) = temp_logger();
        let reporter = AdReportGenerator::new(&logger);
        let output_file = dir.path().join("empty.csv");

        let result = reporter.generate_multi_ad_report(&[], "01-10-26", "01-20-26", &output_file, ReportFormat::Csv);
        assert!(result.is_none());
    }

    #[test]
    fn report_format_from_str_loose() {
        assert_eq!(ReportFormat::from_str_loose("csv"), Some(ReportFormat::Csv));
        assert_eq!(ReportFormat::from_str_loose("CSV"), Some(ReportFormat::Csv));
        assert_eq!(ReportFormat::from_str_loose("pdf"), Some(ReportFormat::Pdf));
        assert_eq!(ReportFormat::from_str_loose("PDF"), Some(ReportFormat::Pdf));
        assert_eq!(ReportFormat::from_str_loose("xyz"), None);
    }

    #[test]
    fn sanitize_filename_replaces_special_chars() {
        assert_eq!(sanitize_filename("Ad Alpha"), "Ad_Alpha");
        assert_eq!(sanitize_filename("test/ad:1"), "test_ad_1");
        assert_eq!(sanitize_filename("normal-name_2"), "normal-name_2");
    }

    #[test]
    fn pdf_bytes_are_valid() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        assert!(!results.is_empty());

        let pdf_bytes = std::fs::read(&results[0].pdf_path).unwrap();
        // PDF files start with %PDF
        assert!(pdf_bytes.starts_with(b"%PDF"), "Not a valid PDF file");
    }

    #[test]
    fn file_naming_convention() {
        let (logger, dir) = temp_logger();
        seed_data(&logger);

        let reporter = AdReportGenerator::new(&logger);
        let output = dir.path().join("reports");
        std::fs::create_dir_all(&output).unwrap();

        let results = reporter.generate_report("01-10-26", "01-20-26", None, &output);
        for r in &results {
            let csv_name = r.csv_path.file_name().unwrap().to_str().unwrap();
            let pdf_name = r.pdf_path.file_name().unwrap().to_str().unwrap();
            assert!(csv_name.starts_with("REPORT_"), "CSV name should start with REPORT_");
            assert!(csv_name.ends_with(".csv"), "CSV name should end with .csv");
            assert!(pdf_name.starts_with("REPORT_"), "PDF name should start with REPORT_");
            assert!(pdf_name.ends_with(".pdf"), "PDF name should end with .pdf");
        }
    }
}
