//! Reads the text on a picture of the Mac's screen with Windows' built-in text recognition,
//! the same engine screen readers use. Easy install uses it to see where the installer is.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

/// One recognised word and where it is, in the Mac's own screen pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Word {
    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub words: Vec<Word>,
}

/// Everything recognised on one picture of the Mac's screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenText {
    pub width: u32,
    pub height: u32,
    pub lines: Vec<Line>,
}

impl ScreenText {
    /// Whether `phrase` appears on the screen, ignoring case and the spacing between words.
    pub fn contains(&self, phrase: &str) -> bool {
        self.find(phrase).is_some()
    }

    /// The first word of `phrase` where the whole phrase appears, ignoring case. A phrase may
    /// span several words of one line, such as "Macintosh HD".
    pub fn find(&self, phrase: &str) -> Option<&Word> {
        self.find_all(phrase).into_iter().next()
    }

    /// Every place `phrase` appears, top to bottom.
    pub fn find_all(&self, phrase: &str) -> Vec<&Word> {
        let wanted: Vec<String> = phrase.split_whitespace().map(simplify).collect();
        let mut found = Vec::new();
        if wanted.is_empty() {
            return found;
        }
        for line in &self.lines {
            for start in 0..line.words.len().saturating_sub(wanted.len() - 1) {
                let window = &line.words[start..start + wanted.len()];
                if window.iter().zip(&wanted).all(|(word, wanted)| simplify(&word.text) == *wanted) {
                    found.push(&window[0]);
                }
            }
        }
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        found
    }

    /// Every word that is exactly `text`, capitals included, top to bottom. For buttons and
    /// menu titles, whose names also turn up in lower case inside sentences.
    pub fn find_exact(&self, text: &str) -> Vec<&Word> {
        let mut found: Vec<&Word> = self.lines.iter().flat_map(|line| &line.words).filter(|word| word.text == text).collect();
        found.sort_by(|a, b| a.y.total_cmp(&b.y));
        found
    }

    /// All the text, one line per recognised line.
    pub fn text(&self) -> String {
        self.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n")
    }
}

/// Lower case with punctuation dropped, so that "Continue…" and "continue" compare equal.
fn simplify(word: &str) -> String {
    word.chars().filter(|character| character.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Reads the text in a PPM picture, the format QEMU's `screendump` writes.
pub fn read_ppm(path: &Path) -> Result<ScreenText> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (width, height, pixels) = decode_ppm(&data).ok_or_else(|| anyhow!("{} is not a PPM picture", path.display()))?;
    // macOS's small text is at the edge of what the engine reads; doubling the picture fixes that.
    let (mut bgra, big_width, big_height) = double_to_bgra(pixels, width, height);
    let mut text = recognise(&bgra, big_width, big_height)?;
    // The engine misses pale text on grey buttons, such as Agree in the installer's dark
    // windows. A second reading of a harsher picture finds those; only its new words are kept.
    sharpen_contrast(&mut bgra);
    if let Ok(harsh) = recognise(&bgra, big_width, big_height) {
        add_new_words(&mut text, harsh);
    }
    for word in text.lines.iter_mut().flat_map(|line| line.words.iter_mut()) {
        word.x /= 2.0;
        word.y /= 2.0;
        word.width /= 2.0;
        word.height /= 2.0;
    }
    text.width = width;
    text.height = height;
    Ok(text)
}

/// Splits a binary PPM into its size and its RGB pixels.
pub fn decode_ppm(data: &[u8]) -> Option<(u32, u32, &[u8])> {
    // Header: "P6", width, height and the maximum value, separated by whitespace, then one whitespace byte.
    let mut position = 0;
    let mut fields = Vec::new();
    while fields.len() < 4 {
        while data.get(position)?.is_ascii_whitespace() {
            position += 1;
        }
        let start = position;
        while !data.get(position)?.is_ascii_whitespace() {
            position += 1;
        }
        fields.push(std::str::from_utf8(&data[start..position]).ok()?);
    }
    position += 1;
    if fields[0] != "P6" || fields[3] != "255" {
        return None;
    }
    let (width, height): (u32, u32) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let pixels = data.get(position..position + (width as usize * height as usize * 3))?;
    Some((width, height, pixels))
}

/// Doubles an RGB picture in both directions, blending neighbours, and returns it as BGRA.
fn double_to_bgra(rgb: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let (width, height) = (width as usize, height as usize);
    let mut out = vec![255u8; width * 2 * height * 2 * 4];
    let pixel = |x: usize, y: usize| {
        let offset = (y.min(height - 1) * width + x.min(width - 1)) * 3;
        [rgb[offset] as u16, rgb[offset + 1] as u16, rgb[offset + 2] as u16]
    };
    for y in 0..height * 2 {
        for x in 0..width * 2 {
            let (a, b, c, d) = (pixel(x / 2, y / 2), pixel(x / 2 + x % 2, y / 2), pixel(x / 2, y / 2 + y % 2), pixel(x / 2 + x % 2, y / 2 + y % 2));
            let offset = (y * width * 2 + x) * 4;
            for channel in 0..3 {
                // BGRA order: the red channel of the source goes third.
                out[offset + 2 - channel] = ((a[channel] + b[channel] + c[channel] + d[channel]) / 4) as u8;
            }
        }
    }
    (out, (width * 2) as u32, (height * 2) as u32)
}

/// Turns a BGRA picture into dark-on-light greys with the bright end stretched: white text
/// becomes black, and a mid-grey button behind it becomes nearly white.
fn sharpen_contrast(bgra: &mut [u8]) {
    for pixel in bgra.chunks_exact_mut(4) {
        let grey = (pixel[0] as u32 * 114 + pixel[1] as u32 * 587 + pixel[2] as u32 * 299) / 1000;
        let value = (255 - grey * grey / 255) as u8;
        pixel[..3].fill(value);
    }
}

/// Adds the words of `extra` that `text` does not already have in the same place.
fn add_new_words(text: &mut ScreenText, extra: ScreenText) {
    let known: Vec<Word> = text.lines.iter().flat_map(|line| line.words.iter().cloned()).collect();
    let is_known = |word: &Word| known.iter().any(|other| (other.x - word.x).abs() < other.width.max(word.width) && (other.y - word.y).abs() < other.height.max(word.height));
    for line in extra.lines {
        let words: Vec<Word> = line.words.into_iter().filter(|word| !is_known(word)).collect();
        if !words.is_empty() {
            let text_of_line = words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
            text.lines.push(Line { text: text_of_line, words });
        }
    }
}

#[cfg(windows)]
fn recognise(bgra: &[u8], width: u32, height: u32) -> Result<ScreenText> {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let writer = DataWriter::new()?;
    writer.WriteBytes(bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(&buffer, BitmapPixelFormat::Bgra8, width as i32, height as i32, BitmapAlphaMode::Ignore)?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().context("Windows has no text recognition language installed")?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    let mut lines = Vec::new();
    for line in result.Lines()? {
        let mut words = Vec::new();
        for word in line.Words()? {
            let bounds = word.BoundingRect()?;
            words.push(Word { text: word.Text()?.to_string(), x: bounds.X, y: bounds.Y, width: bounds.Width, height: bounds.Height });
        }
        lines.push(Line { text: line.Text()?.to_string(), words });
    }
    Ok(ScreenText { width, height, lines })
}

#[cfg(not(windows))]
fn recognise(_bgra: &[u8], _width: u32, _height: u32) -> Result<ScreenText> {
    Err(anyhow!("text recognition needs Windows"))
}

/// Whether Windows has a text recognition language installed. Easy install reads the Mac's
/// screen with it.
pub fn available() -> bool {
    #[cfg(windows)]
    {
        windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages().is_ok()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Line, ScreenText, Word, add_new_words, decode_ppm, double_to_bgra, sharpen_contrast};

    #[test]
    fn a_second_reading_adds_only_words_in_new_places() {
        let mut first = ScreenText { width: 100, height: 100, lines: vec![line(10.0, &["Disagree"])] };
        let mut second_line = line(10.0, &["Disagree", "Agree"]);
        second_line.words[0].x += 3.0;
        add_new_words(&mut first, ScreenText { width: 100, height: 100, lines: vec![second_line, line(50.0, &["Continue"])] });
        let words: Vec<&str> = first.lines.iter().flat_map(|line| &line.words).map(|word| word.text.as_str()).collect();
        assert_eq!(words, vec!["Disagree", "Agree", "Continue"]);
    }

    #[test]
    fn harsher_contrast_makes_pale_text_dark_and_grey_buttons_light() {
        let mut pixels = vec![230, 230, 230, 255, 90, 90, 90, 255, 0, 0, 0, 255];
        sharpen_contrast(&mut pixels);
        assert!(pixels[0] < 60, "pale text became {}", pixels[0]);
        assert!(pixels[4] > 200, "the button became {}", pixels[4]);
        assert_eq!(pixels[8], 255);
    }

    fn line(y: f32, words: &[&str]) -> Line {
        let words: Vec<Word> = words.iter().enumerate().map(|(index, text)| Word { text: text.to_string(), x: index as f32 * 50.0, y, width: 40.0, height: 10.0 }).collect();
        Line { text: words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" "), words }
    }

    #[test]
    fn a_phrase_is_found_across_words_whatever_its_case_or_punctuation() {
        let screen = ScreenText { width: 100, height: 100, lines: vec![line(60.0, &["Agree"]), line(10.0, &["Reinstall", "macOS", "Big", "Sur"]), line(30.0, &["Continue…"])] };
        assert_eq!(screen.find("reinstall macos").map(|word| word.text.as_str()), Some("Reinstall"));
        assert!(screen.contains("continue"));
        assert!(!screen.contains("Disagree"));
        assert!(!screen.contains("macOS Monterey"));
    }

    #[test]
    fn matches_come_back_top_to_bottom() {
        let screen = ScreenText { width: 100, height: 100, lines: vec![line(80.0, &["Agree"]), line(20.0, &["Agree"])] };
        let found: Vec<f32> = screen.find_all("Agree").iter().map(|word| word.y).collect();
        assert_eq!(found, vec![20.0, 80.0]);
    }

    #[test]
    fn a_ppm_is_split_into_size_and_pixels_and_doubled() {
        let mut data = b"P6\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[10, 20, 30, 50, 60, 70]);
        let (width, height, pixels) = decode_ppm(&data).unwrap();
        assert_eq!((width, height, pixels.len()), (2, 1, 6));
        let (bgra, big_width, big_height) = double_to_bgra(pixels, width, height);
        assert_eq!((big_width, big_height, bgra.len()), (4, 2, 32));
        // The first pixel keeps its colour, in blue-green-red-alpha order.
        assert_eq!(&bgra[0..4], &[30, 20, 10, 255]);
        // The second is half way between the two source pixels.
        assert_eq!(&bgra[4..8], &[50, 40, 30, 255]);
        assert!(decode_ppm(b"P6\n2 1\n255\n\x01").is_none());
    }
}
