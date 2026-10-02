//! 検出精度の回帰テスト。
//! - 陰性: 個人情報を含まない一般的な文章で誤検出しないこと
//! - 陽性: 典型的な書き方の機密情報を漏らさないこと

use sumiveil_core::config::Config;
use sumiveil_core::engine::mask_text;

const NEGATIVE_JA: &str = "\
本日の定例会議は午後3時から第二会議室で行います。
議題は来期の予算計画と、新製品のリリーススケジュールについてです。
資料は共有フォルダの「2026年度計画」フォルダに保存しました。
前回の議事録の3ページ目、第4項を確認してください。
売上は前年同期比で12%増加し、目標の95%を達成しました。
バージョン1.2.3で不具合が修正され、ビルド番号は2045です。
東京駅から徒歩5分の会場で、参加者は約30名の予定です。
お客様各位、いつもご利用いただきありがとうございます。
皆様のご協力のおかげで、プロジェクトは順調に進んでいます。
仕様書の第2章を更新しました。ご確認をお願いします。
システムのメンテナンスは土曜日の深夜に実施します。
申請様式は総務部のポータルからダウンロードできます。
品質管理部では、検査手順の見直しを進めています。
";

const NEGATIVE_EN: &str = "\
The quarterly review meeting is scheduled for Thursday afternoon.
Please review the attached proposal and share your feedback by Friday.
Revenue grew 12% year over year, reaching 95% of the annual target.
Version 2.4.1 fixes the crash reported in issue 1234.
The build pipeline runs unit tests, integration tests and linting.
Our team will present the roadmap at the next all-hands meeting.
Hello World! This is a simple example program written in Rust.
Hi team, thanks for the great work on the release.
See section 3.2 of the design document for details on caching.
Use std::io::Read and serde_json::from_str to parse the file.
";

/// 大文字で始まる語が並ぶ英語 (見出し・製品名・日本由来の語)。ローマ字の人名と取り違えないこと
const NEGATIVE_EN_TITLES: &str = "\
Go To Market Strategy
Getting Started With Release Notes
Table Of Contents
Make Sure You Save As Before You Log Out
Take Home Pay And Tax Rates
Tokyo Office Moves To Kyoto University Campus
Kato Comet and Mori Breeze Sales Report
Tanuki Switch Online Service
Visual Editor Code Remote Development
Sushi Bar And Ramen Shop Near Osaka Castle
Kobe Beef Dinner At Nara Park
Anime Expo Opening Day
Karaoke Night With The Data Science Team
Mount Fuji Climbing Guide
Hide Details / Show More / Sign In
Sake Tasting Event Schedule
Zen Mode For Your Terminal
Yoga Class Every Monday
Token Refresh Interval Settings
Sudo Access Request Form
";

/// ソースコード (項目名と同じ語の定数・フィールドや型注釈)
const NEGATIVE_CODE: &str = "\
use mio::{Events, Interest, Poll, Token};

// Some tokens to allow us to identify which event is for which socket.
const SERVER: Token = Token(0);
const CLIENT: Token = Token(1);

struct Message {
    from: Option<String>,
    to: String,
    owner: String,
    author: Box<Person>,
}
let contact: Contact = Contact::new();
fn reviewer(name: Name) -> Name { name }
fn parse(token: &str) -> Option<i32> { None }
fn hash_password<D: Digest>(password: &str, salt: &str) {}
hash_password::<sha2::Sha256>(input, salt);
let token = match next() { Some(t) => t, None => return };
println!(\"token: {}\", token.text);
token: 東京スカイツリー, details: [名詞, 固有名詞]
";

fn detections(text: &str) -> Vec<String> {
    let r = mask_text(&Config::default(), text);
    r.replacements.iter().map(|x| format!("{}: {}", x.meta.id, x.original)).collect()
}

#[test]
fn no_false_positives_in_ordinary_japanese() {
    let d = detections(NEGATIVE_JA);
    assert!(d.is_empty(), "false positives: {d:#?}");
}

#[test]
fn no_false_positives_in_ordinary_english() {
    let d = detections(NEGATIVE_EN);
    assert!(d.is_empty(), "false positives: {d:#?}");
}

#[test]
fn no_false_positives_in_source_code() {
    let d = detections(NEGATIVE_CODE);
    assert!(d.is_empty(), "false positives: {d:#?}");
}

#[test]
fn no_false_positives_in_english_titles() {
    let d = detections(NEGATIVE_EN_TITLES);
    assert!(d.is_empty(), "false positives: {d:#?}");
}

#[test]
fn no_false_positives_with_morphology() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dict/ipadic");
    if !dir.join("metadata.json").is_file() {
        eprintln!("skip: morphology dictionary not built");
        return;
    }
    let mut cfg = Config::default();
    cfg.names.use_morphology = true;
    cfg.names.morphology_dict = dir.display().to_string();
    for text in [NEGATIVE_JA, NEGATIVE_EN, NEGATIVE_EN_TITLES, NEGATIVE_CODE] {
        let r = mask_text(&cfg, text);
        let d: Vec<String> = r.replacements.iter().map(|x| format!("{}: {}", x.meta.id, x.original)).collect();
        assert!(d.is_empty(), "false positives with morphology: {d:#?}");
    }
}

/// 実在する珍しい姓 (内蔵の姓辞書にないもの)。珍しい名字の紹介サイトなどで調べたもの。
const RARE_SURNAMES: &[&str] = &[
    "小鳥遊", "月見里", "五百旗頭", "栗花落", "雲類鷲", "四十八願", "七五三掛", "大豆生田", "甘露寺", "久寿米木", "阿比留", "一寸木",
    "五百蔵", "十八女", "漆真下", "温泉川", "貫地谷", "岡田垣内", "上中別府", "信濃小路", "西四辻", "滋野井", "鵜久森", "瀧野瀬",
];

#[test]
fn rare_surnames_do_not_leak() {
    // 敬称・項目名があれば珍しい姓も検出し、「姓 名さん」でも名だけをマスクして姓が残ることはない
    let given = ["太郎", "花子", "健一", "美咲"];
    for (i, s) in RARE_SURNAMES.iter().enumerate() {
        let g = given[i % given.len()];
        for text in [
            format!("{s}様にご確認いただきました。"),
            format!("担当: {s} {g}"),
            format!("{s} {g}さんから連絡がありました。"),
            format!("{s}{g}様宛の書類です。"),
            // 敬称がなくても、辞書の名と並んでいれば検出する
            format!("本日、{s} {g}が来社しました。"),
            format!("本日、{s}{g}が来社しました。"),
        ] {
            let r = mask_text(&Config::default(), &text);
            assert!(!r.output.contains(s), "surname leaked: {text} → {}", r.output);
        }
    }
    // 地名・日付 + 名に見える一般語は人名にしない
    let d = detections("北海道大地を走る鉄道の旅。日本未来会議の資料。本日 未来について議論します。大阪純一郎ビル。社会正義を実現。");
    assert!(d.iter().all(|x| !x.starts_with("person_name")), "false positives: {d:#?}");
}

#[test]
fn positives_do_not_leak() {
    let cases: &[(&str, &str)] = &[
        ("担当の佐々木です。", "佐々木"),
        ("田中 美咲 様へ", "美咲"),
        ("From: Jane Smith <jane.smith@example.org>", "Jane"),
        ("Contact: Michael Brown", "Brown"),
        ("電話番号：０３－１２３４－５６７８", "１２３４"),
        ("携帯 080 1234 5678", "5678"),
        ("mail: hanako_s+test@mail.example2.jp", "hanako_s"),
        ("接続先 172.16.254.1:8080", "172.16.254.1"),
        ("IPv6 fe80::1ff:fe23:4567:890a", "fe80::1ff"),
        ("住所 大阪府大阪市北区架空町1-2-3", "架空町"),
        ("〒123-4567 大阪市北区架空町3丁目1番1号", "架空町"),
        ("口座番号: 7654321", "7654321"),
        ("VISA 4012 8888 8888 1881", "8888"),
        ("export GITHUB_TOKEN=ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ", "ghp_"),
        ("DATABASE_URL=mysql://app:p4ssw0rd@db.internal:3306/prod", "p4ssw0rd"),
        ("api_key: \"AbCdEf123456\"", "AbCdEf123456"),
        ("token=MQ1BBEZ3UC", "MQ1BBEZ3UC"),
        ("auth_token: xyz", "xyz"),
        ("Authorization token: 9f8e7d6c5b4a3210", "9f8e7d6c5b4a3210"),
        ("パスワードは Summer2026! です", "Summer2026"),
        ("C:\\Users\\hanako.sato\\Downloads", "hanako.sato"),
        ("社員番号：E-102938", "102938"),
        ("生年月日 昭和60年4月1日", "昭和60年"),
        ("株式会社テスト技研の皆様", "テスト技研"),
        // ローマ字の日本人名
        ("Best regards, Hanako Suzuki", "Hanako"),
        ("Shota Takahashi joined the team.", "Takahashi"),
        ("Suzuki Hanako will attend.", "Hanako"),
        ("YAMADA Taro", "YAMADA"),
        ("Taro YAMADA", "Taro"),
        ("Meeting with Tanaka-san tomorrow", "Tanaka"),
        ("Sato san will join", "Sato"),
        ("Dear Mr. Yamada,", "Yamada"),
        ("Name: Kenji Watanabe", "Watanabe"),
        ("Please contact Satoh Yuuta.", "Satoh"),
        ("Ryouko Oono and Shinichi Homma", "Homma"),
    ];
    let mut leaks = vec![];
    for (text, secret) in cases {
        let r = mask_text(&Config::default(), text);
        if r.output.contains(secret) {
            leaks.push(format!("{text:?} -> {:?}", r.output));
        }
    }
    assert!(leaks.is_empty(), "leaked:\n{}", leaks.join("\n"));
}
