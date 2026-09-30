//! テキスト以外の形式 (Office 文書・メール) の読み込みと書き戻し。テスト用のファイルはここで組み立てる (ダミーデータのみ)。

use std::io::{Cursor, Read, Write};
use std::path::Path;

use sumiveil_core::config::Config;
use sumiveil_core::formats::{self, DocKind, Output, PropertiesMode, WriteOptions};
use sumiveil_core::{Engine, MaskSession};

fn make_zip(files: &[(&str, &str)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in files {
        z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn unzip_all(bytes: &[u8]) -> Vec<(String, String)> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).expect("output must be a valid zip");
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            (f.name().to_string(), s)
        })
        .collect()
}

/// 開く → マスク → 書き出す。
fn roundtrip(name: &str, bytes: Vec<u8>, mode: PropertiesMode) -> (formats::Document, String, Output, formats::WriteReport) {
    let doc = formats::open(Path::new(name), bytes, None).unwrap();
    let engine = Engine::new(&Config::default());
    let mut session = MaskSession::default();
    let result = engine.mask(&doc.text, &mut session);
    let mut opts = WriteOptions { engine: &engine, session: &mut session, properties: mode, text_encoding: None };
    let (out, report) = formats::write(&doc, &result, &mut opts).unwrap();
    (doc, result.output, out, report)
}

fn bytes(o: &Output) -> &[u8] {
    match o {
        Output::Bytes(b) => b,
        Output::Text(t) => t.as_bytes(),
    }
}

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn sample_docx() -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {W}><w:body>
<w:p><w:r><w:t xml:space="preserve">担当: </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>山</w:t></w:r><w:r><w:t>田太郎様</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">メール: taro.yamada@corp.example3.co.jp &amp; 電話 090-1234-5678</w:t></w:r></w:p>
<w:p><w:ins w:id="1" w:author="佐藤花子" w:date="2026-01-01T00:00:00Z"><w:r><w:t>追記しました</w:t></w:r></w:ins></w:p>
</w:body></w:document>"#
    );
    let comments = format!(r#"<w:comments {W}><w:comment w:id="0" w:author="鈴木一郎" w:initials="SI"><w:p><w:r><w:t>山田様に確認</w:t></w:r></w:p></w:comment></w:comments>"#);
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="mailto:hanako.sato@corp.example3.co.jp" TargetMode="External"/></Relationships>"#;
    let core = r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>山田太郎</dc:creator><cp:lastModifiedBy>佐藤花子</cp:lastModifiedBy><dc:title>見積書</dc:title></cp:coreProperties>"#;
    let app = r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Company>株式会社サンプル商事</Company></Properties>"#;
    make_zip(&[
        ("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#),
        ("word/document.xml", &doc),
        ("word/comments.xml", &comments),
        ("word/_rels/document.xml.rels", rels),
        ("docProps/core.xml", core),
        ("docProps/app.xml", app),
    ])
}

#[test]
fn docx_masks_body_comments_links_and_clears_properties() {
    let (doc, masked, out, report) = roundtrip("report.docx", sample_docx(), PropertiesMode::Clear);
    assert_eq!(doc.kind, DocKind::Docx);
    assert!(doc.text.contains("担当: 山田太郎様"), "{}", doc.text);
    assert!(doc.warnings.iter().any(|w| w.contains("変更履歴")));
    assert!(doc.properties.iter().any(|p| p.label == "作成者" && p.value == "山田太郎"));
    assert!(doc.properties.iter().any(|p| p.value == "鈴木一郎"));
    assert!(masked.contains("<NAME_1>様"), "{masked}");
    assert_eq!(report.properties, Some("cleared"));
    let parts = unzip_all(bytes(&out));
    let all: String = parts.iter().map(|(_, s)| s.as_str()).collect();
    for secret in ["山田", "田太郎", "taro.yamada", "090-1234-5678", "佐藤花子", "鈴木一郎", "hanako.sato", "サンプル商事"] {
        assert!(!all.contains(secret), "{secret} leaked:\n{all}");
    }
    let body = &parts.iter().find(|(n, _)| n == "word/document.xml").unwrap().1;
    // 書式 (太字) と、マスクしなかった部分のエスケープはそのまま
    assert!(body.contains("<w:b/>"));
    assert!(body.contains("&amp; 電話"));
    // 分かれていた run: 先頭の run に置換後の文字、残りの run からは取り除く
    assert!(body.contains("<w:t>&lt;NAME_1&gt;</w:t>") && body.contains("<w:t>様</w:t>"), "{body}");
    // マスクしていない部品 ([Content_Types].xml) は元のまま
    assert!(parts.iter().any(|(n, s)| n == "[Content_Types].xml" && s.contains("content-types")));
}

#[test]
fn docx_property_modes() {
    let (_, _, out, report) = roundtrip("a.docx", sample_docx(), PropertiesMode::Keep);
    assert_eq!(report.properties, Some("kept"));
    let all: String = unzip_all(bytes(&out)).into_iter().map(|(_, s)| s).collect();
    assert!(all.contains("<cp:lastModifiedBy>佐藤花子</cp:lastModifiedBy>"));
    let (_, _, out, report) = roundtrip("a.docx", sample_docx(), PropertiesMode::Mask);
    assert_eq!(report.properties, Some("masked"));
    let core = unzip_all(bytes(&out)).into_iter().find(|(n, _)| n == "docProps/core.xml").unwrap().1;
    // 本文と同じ連番 (山田太郎 = NAME_1)
    assert!(core.contains("<dc:creator>&lt;NAME_1&gt;</dc:creator>"), "{core}");
}

fn sample_xlsx() -> Vec<u8> {
    let wb = r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="顧客一覧" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let wb_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;
    let ss = r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3"><si><t>氏名</t></si><si><t>山田太郎</t><rPh sb="0" eb="2"><t>ヤマダタロウ</t></rPh></si><si><t>電話</t></si><si><t>未使用: 佐藤花子様</t></si></sst>"#;
    let sheet = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>2</v></c></row>
<row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2" t="inlineStr"><is><t>090-1234-5678</t></is></c><c r="C2"><v>9012345678</v></c></row>
</sheetData></worksheet>"#;
    make_zip(&[
        ("xl/workbook.xml", wb),
        ("xl/_rels/workbook.xml.rels", wb_rels),
        ("xl/sharedStrings.xml", ss),
        ("xl/worksheets/sheet1.xml", sheet),
        ("docProps/core.xml", r#"<cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:creator>山田太郎</dc:creator></cp:coreProperties>"#),
    ])
}

#[test]
fn xlsx_masks_cells_phonetic_and_unused_strings() {
    let (doc, masked, out, _) = roundtrip("list.xlsx", sample_xlsx(), PropertiesMode::Clear);
    assert!(doc.text.contains("氏名\t電話\n山田太郎\t090-1234-5678"), "{}", doc.text);
    assert_eq!(doc.location_at(doc.text.find("090-").unwrap()), Some("顧客一覧!B2"));
    assert!(doc.warnings.iter().any(|w| w.contains("数値")));
    assert!(masked.contains("<PHONE_1>"));
    let parts = unzip_all(bytes(&out));
    let all: String = parts.iter().map(|(_, s)| s.as_str()).collect();
    for secret in ["山田太郎", "ヤマダ", "090-1234-5678", "佐藤花子"] {
        assert!(!all.contains(secret), "{secret} leaked:\n{all}");
    }
    // 数式から参照されるシート名は変えない
    assert!(all.contains(r#"name="顧客一覧""#));
}

#[test]
fn pptx_masks_slides_notes_and_comment_authors() {
    let slide = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>ご担当: 山田太郎様</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
    let notes = r#"<p:notes xmlns:p="p" xmlns:a="a"><a:p><a:r><a:t>連絡先 090-1234-5678</a:t></a:r></a:p></p:notes>"#;
    let authors = r#"<p:cmAuthorLst xmlns:p="p"><p:cmAuthor id="0" name="佐藤花子" initials="SH" lastIdx="1" clrIdx="0"/></p:cmAuthorLst>"#;
    let bytes_in = make_zip(&[
        ("ppt/presentation.xml", "<p:presentation xmlns:p=\"p\"/>"),
        ("ppt/slides/slide1.xml", slide),
        ("ppt/notesSlides/notesSlide1.xml", notes),
        ("ppt/commentAuthors.xml", authors),
    ]);
    let (doc, _, out, _) = roundtrip("deck.pptx", bytes_in, PropertiesMode::Clear);
    assert!(doc.text.contains("山田太郎様") && doc.text.contains("090-1234-5678"));
    assert_eq!(doc.location_at(doc.text.find("090").unwrap()), Some("ノート 1"));
    let all: String = unzip_all(bytes(&out)).into_iter().map(|(_, s)| s).collect();
    for secret in ["山田太郎", "090-1234-5678", "佐藤花子"] {
        assert!(!all.contains(secret), "{secret} leaked:\n{all}");
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    data.chunks(3)
        .flat_map(|c| {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            [T[(n >> 18) as usize & 63] as char, T[(n >> 12) as usize & 63] as char, if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' }, if c.len() > 2 { T[n as usize & 63] as char } else { '=' }]
        })
        .collect()
}

#[test]
fn eml_iso2022jp_is_decoded_and_rewritten() {
    let jis = |s: &str| encoding_rs::ISO_2022_JP.encode(s).0.into_owned();
    let subject = format!("=?ISO-2022-JP?B?{}?=", b64(&jis("山田太郎様 お見積りの件")));
    let from_name = format!("=?ISO-2022-JP?B?{}?=", b64(&jis("鈴木一郎")));
    let body = b64(&jis("山田太郎様\r\n\r\nお世話になっております。電話 090-1234-5678 までご連絡ください。\r\n"));
    let raw = format!(
        "Received: from mail.corp.example3.co.jp (10.20.30.40)\r\nFrom: {from_name} <ichiro.suzuki@corp.example3.co.jp>\r\nTo: taro.yamada@corp.example3.co.jp\r\nSubject: {subject}\r\nDate: Mon, 1 Jun 2026 10:00:00 +0900\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"XX\"\r\n\r\n--XX\r\nContent-Type: text/plain; charset=ISO-2022-JP\r\nContent-Transfer-Encoding: base64\r\n\r\n{body}\r\n--XX\r\nContent-Type: application/pdf; name=\"yamada_contract.pdf\"\r\nContent-Disposition: attachment; filename=\"yamada_contract.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0=\r\n--XX--\r\n"
    );
    let (doc, masked, out, _) = roundtrip("mail.eml", raw.into_bytes(), PropertiesMode::Clear);
    assert!(doc.text.contains("件名: 山田太郎様 お見積りの件"), "{}", doc.text);
    assert!(doc.text.contains("090-1234-5678"));
    assert!(doc.warnings.iter().any(|w| w.contains("添付ファイル")));
    assert!(masked.contains("<PHONE_1>"));
    let out = String::from_utf8(bytes(&out).to_vec()).unwrap();
    assert!(!out.contains("Received") && !out.contains("10.20.30.40") && !out.contains("JVBERi0"));
    // 書き出したメールを読み直すと、マスク済みの件名・本文・宛先になっている
    let msg = mail_parser::MessageParser::default().parse(out.as_bytes()).unwrap();
    let subject = msg.subject().unwrap();
    assert!(subject.starts_with("<NAME_1>様"), "{subject}");
    let body = msg.body_text(0).unwrap();
    assert!(body.contains("<PHONE_1>") && !body.contains("090-1234"), "{body}");
    let whole = format!("{subject}{body}{:?}{:?}", msg.from(), msg.to());
    for secret in ["山田", "鈴木", "taro.yamada", "ichiro.suzuki", "yamada_contract"] {
        assert!(!whole.contains(secret) && !out.contains(secret), "{secret} leaked:\n{out}");
    }
    assert_eq!(msg.attachment_count(), 0);
}

#[test]
fn msg_is_read_as_text() {
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    let mut put = |path: &str, s: &str| {
        let mut st = cf.create_stream(path).unwrap();
        let b: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        st.write_all(&b).unwrap();
    };
    put("/__substg1.0_0037001F", "山田太郎様への見積り");
    put("/__substg1.0_0C1A001F", "鈴木一郎");
    put("/__substg1.0_5D01001F", "ichiro.suzuki@corp.example3.co.jp");
    put("/__substg1.0_1000001F", "山田太郎様\r\n電話 090-1234-5678 までお願いします。");
    cf.create_storage("/__attach_version1.0_#00000000").unwrap();
    let mut st = cf.create_stream("/__attach_version1.0_#00000000/__substg1.0_3707001F").unwrap();
    st.write_all(&"見積書_山田.xlsx".encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    drop(st);
    let data = cf.into_inner().into_inner();
    let (doc, masked, out, _) = roundtrip("mail.msg", data, PropertiesMode::Clear);
    assert!(doc.text.contains("件名: 山田太郎様への見積り"), "{}", doc.text);
    assert!(doc.text.contains("鈴木一郎 <ichiro.suzuki@corp.example3.co.jp>"));
    assert!(doc.text.contains("見積書_山田.xlsx"));
    let Output::Text(t) = out else { panic!("msg must be written as text") };
    assert_eq!(t, masked);
    for secret in ["山田太郎", "ichiro.suzuki", "090-1234-5678"] {
        assert!(!t.contains(secret), "{secret} leaked:\n{t}");
    }
}

#[test]
fn pdf_is_extracted_per_page_and_written_as_text() {
    // tests/fixtures/sample-ja.pdf: ダミーデータの HTML を Microsoft Edge で PDF にしたもの (2 ページ)
    let data = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-ja.pdf")).unwrap();
    let (doc, masked, out, _) = roundtrip("quote.pdf", data, PropertiesMode::Clear);
    assert!(doc.text.contains("山田太郎"), "{}", doc.text);
    assert!(doc.text.contains("090-1234-5678"), "{}", doc.text);
    assert_eq!(doc.location_at(doc.text.find("鈴木").unwrap()), Some("2 ページ"));
    let Output::Text(t) = out else { panic!("pdf must be written as text") };
    assert_eq!(t, masked);
    for secret in ["山田太郎", "taro.yamada", "090-1234-5678", "ichiro.suzuki", "Hanako Suzuki"] {
        assert!(!t.contains(secret), "{secret} leaked:\n{t}");
    }
}

#[test]
fn text_files_keep_encoding() {
    let sjis = encoding_rs::SHIFT_JIS.encode("山田太郎様 090-1234-5678").0.into_owned();
    let (doc, _, out, _) = roundtrip("log.txt", sjis, PropertiesMode::Clear);
    assert_eq!(doc.encoding_name, "Shift_JIS");
    let back = encoding_rs::SHIFT_JIS.decode(bytes(&out)).0.into_owned();
    assert_eq!(back, "<NAME_1>様 <PHONE_1>");
}

#[test]
fn broken_files_are_errors_not_panics() {
    for name in ["a.docx", "a.xlsx", "a.pptx", "a.pdf", "a.msg"] {
        assert!(formats::open(Path::new(name), b"not a real file".to_vec(), None).is_err(), "{name}");
    }
    let old_doc = [0xD0u8, 0xCF, 0x11, 0xE0, 0, 0, 0, 0].to_vec();
    assert!(formats::open(Path::new("a.docx"), old_doc, None).unwrap_err().contains("パスワード"));
}
