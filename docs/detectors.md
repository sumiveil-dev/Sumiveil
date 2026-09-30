# 検出器一覧 / Detectors

> このファイルは `cargo run -p sumiveil-core --example gen-docs` で自動生成されます。

- 検出器数: 67
- カテゴリ単位 (`categories.<id>.enabled`) と検出器単位 (`detectors.<id>.enabled`) で有効/無効を切り替えられます。
- 「既定」列が ✓ のものは初期状態で有効です。

## 連絡先 (Contact) — `contact`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `email` | メールアドレス<br><sub>Email address</sub> | `EMAIL` | ✓ | 60 | `連絡先: taro.yamada@corp.example.co.jp まで` |
| `phone_jp` | 電話番号 (日本)<br><sub>Phone number (Japan)</sub> | `PHONE` | ✓ | 65 | `TEL: 090-1234-5678` |
| `phone_intl` | 電話番号 (国際)<br><sub>Phone number (international)</sub> | `PHONE` | ✓ | 60 | `Call +1 415-555-0132` |
| `postal_code_jp` | 郵便番号<br><sub>Postal code (Japan)</sub> | `POSTAL` | ✓ | 55 | `〒100-0001 東京都千代田区` |

## 個人情報 (Personal) — `personal`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `person_name` | 人名<br><sub>Person name</sub> | `NAME` | ✓ | 40 | `山田太郎様、お世話になっております。` |
| `address_jp` | 住所 (都道府県から)<br><sub>Address (Japan, with prefecture)</sub> | `ADDRESS` | ✓ | 58 | `東京都千代田区架空町1丁目2-3 サンプルビル5F` |
| `address_jp_city` | 住所 (市区町村から)<br><sub>Address (Japan, city + block number)</sub> | `ADDRESS` | ✓ | 56 | `横浜市中区山下町1-2-3` |
| `address_en` | 住所 (英語表記)<br><sub>Street address (English)</sub> | `ADDRESS` | ✓ | 55 | `Ship to 1600 Example Avenue, Suite 12` |
| `birthdate` | 生年月日<br><sub>Date of birth</sub> | `BIRTHDATE` | ✓ | 57 | `生年月日: 1985年4月1日` |
| `date` | 日付 (すべて)<br><sub>Date (any)</sub> | `DATE` |  | 30 | `2024/03/15 に実施` |
| `age` | 年齢<br><sub>Age</sub> | `AGE` |  | 30 | `年齢 42歳` |
| `employee_id` | 社員番号<br><sub>Employee ID</sub> | `EMPLOYEE_ID` | ✓ | 62 | `社員番号: A12345` |

## 日本の公的ID (Japanese IDs) — `jp_id`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `my_number` | マイナンバー (個人番号)<br><sub>My Number (individual number)</sub> | `MY_NUMBER` | ✓ | 80 | `マイナンバー: 1234 5678 9018` |
| `corporate_number` | 法人番号・インボイス登録番号<br><sub>Corporate number / Invoice registration no.</sub> | `CORP_NUMBER` | ✓ | 78 | `登録番号 T7000012050002` |
| `drivers_license_jp` | 運転免許証番号<br><sub>Driver's license number (Japan)</sub> | `DRIVERS_LICENSE` | ✓ | 80 | `運転免許証番号 301234567890` |
| `passport_jp` | パスポート番号<br><sub>Passport number (Japan)</sub> | `PASSPORT` | ✓ | 80 | `パスポート番号: TK1234567` |
| `residence_card_jp` | 在留カード番号<br><sub>Residence card number (Japan)</sub> | `RESIDENCE_CARD` | ✓ | 80 | `在留カード AB12345678CD` |
| `pension_number_jp` | 基礎年金番号<br><sub>Basic pension number (Japan)</sub> | `PENSION_NO` | ✓ | 80 | `基礎年金番号 1234-567890` |
| `health_insurance_jp` | 健康保険証の記号・番号<br><sub>Health insurance card number (Japan)</sub> | `HEALTH_INSURANCE` | ✓ | 80 | `保険証 記号 1234 番号 56` |
| `bank_account_jp` | 銀行口座番号<br><sub>Bank account (Japan)</sub> | `BANK_ACCOUNT` | ✓ | 80 | `振込先: 普通 1234567` |

## 海外のID (International IDs) — `intl_id`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `us_ssn` | 米国社会保障番号 (SSN)<br><sub>US Social Security Number</sub> | `SSN` | ✓ | 75 | `SSN: 123-45-6789` |
| `iban` | 国際銀行口座番号 (IBAN)<br><sub>IBAN</sub> | `IBAN` | ✓ | 80 | `IBAN GB82 WEST 1234 5698 7654 32` |
| `swift_bic` | SWIFT/BIC コード<br><sub>SWIFT / BIC code</sub> | `SWIFT` | ✓ | 70 | `SWIFT: EXAMJPJT` |

## 金融 (Finance) — `finance`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `credit_card` | クレジットカード番号<br><sub>Credit card number</sub> | `CREDIT_CARD` | ✓ | 85 | `カード番号 4111-1111-1111-1111` |
| `card_expiry` | カード有効期限<br><sub>Card expiry date</sub> | `CARD_EXPIRY` | ✓ | 70 | `有効期限 12/28` |
| `card_cvv` | セキュリティコード (CVV)<br><sub>Card security code (CVV)</sub> | `CVV` | ✓ | 70 | `CVV: 123` |
| `money` | 金額<br><sub>Monetary amount</sub> | `AMOUNT` |  | 35 | `契約金額 1,200万円` |

## ネットワーク/IT (Network / IT) — `network`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `ipv4` | IPv4 アドレス<br><sub>IPv4 address</sub> | `IPV4` | ✓ | 50 | `接続元 192.168.10.25 から` |
| `ipv6` | IPv6 アドレス<br><sub>IPv6 address</sub> | `IPV6` | ✓ | 50 | `addr 2001:db8:85a3::8a2e:370:7334` |
| `mac_address` | MAC アドレス<br><sub>MAC address</sub> | `MAC` | ✓ | 50 | `MAC 00:1A:2B:3C:4D:5E` |
| `url` | URL (全体)<br><sub>URL (entire)</sub> | `URL` |  | 45 | `https://intranet.example.co.jp/wiki/page` |
| `url_credentials` | URL 内の認証情報 (user:pass@)<br><sub>Credentials in URL (user:pass@)</sub> | `URL_CREDENTIALS` | ✓ | 88 | `postgres://admin:S3cretPass@db01:5432/app` |
| `url_secret_params` | URL の秘密パラメータ値 (token= 等)<br><sub>Secret query parameters in URL</sub> | `URL_SECRET` | ✓ | 88 | `https://example.com/cb?code=abc123XYZ&state=1` |
| `hostname` | ホスト名 (FQDN)<br><sub>Host name / FQDN</sub> | `HOST` | ✓ | 45 | `db01.prod.internal に接続` |
| `windows_user_path` | Windows パス内のユーザー名<br><sub>User name in Windows path</sub> | `USER` | ✓ | 60 | `C:\Users\yamada.taro\Documents\report.xlsx` |
| `unix_home_path` | Unix ホームパス内のユーザー名<br><sub>User name in Unix home path</sub> | `USER` | ✓ | 60 | `/home/tyamada/.ssh/config` |
| `unc_path` | UNC パスのサーバー名<br><sub>Server name in UNC path</sub> | `HOST` | ✓ | 60 | `\\fileserver01\share\経理` |
| `domain_user` | ドメイン\ユーザー<br><sub>Domain\User account</sub> | `USER` | ✓ | 55 | `ログオン: CORP\tyamada` |
| `windows_sid` | Windows SID<br><sub>Windows SID</sub> | `SID` | ✓ | 60 | `S-1-5-21-3623811015-3361044348-30300820-1013` |
| `uuid` | UUID / GUID<br><sub>UUID / GUID</sub> | `UUID` |  | 40 | `id=3f2504e0-4f89-11d3-9a0c-0305e82c3301` |

## 認証情報・シークレット (Credentials / Secrets) — `secret`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `private_key` | 秘密鍵ブロック (PEM/SSH/PGP)<br><sub>Private key block (PEM/SSH/PGP)</sub> | `PRIVATE_KEY` | ✓ | 100 | `-----BEGIN RSA PRIVATE KEY----- ⏎ MIIEdummy ⏎ -----END RSA PRIVATE KEY-----` |
| `aws_access_key` | AWS アクセスキー ID<br><sub>AWS access key ID</sub> | `AWS_KEY` | ✓ | 95 | `AKIAIOSFODNN7EXAMPLE` |
| `aws_secret_key` | AWS シークレットアクセスキー<br><sub>AWS secret access key</sub> | `AWS_SECRET` | ✓ | 95 | `aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` |
| `github_token` | GitHub トークン<br><sub>GitHub token</sub> | `GITHUB_TOKEN` | ✓ | 95 | `ghp_0123456789abcdefghijABCDEFGHIJ012345` |
| `gitlab_token` | GitLab トークン<br><sub>GitLab token</sub> | `GITLAB_TOKEN` | ✓ | 95 | `glpat-abcdefghij0123456789` |
| `slack_token` | Slack トークン<br><sub>Slack token</sub> | `SLACK_TOKEN` | ✓ | 95 | `xoxb-1234567890-abcdefghijkl` |
| `slack_webhook` | Slack Webhook URL<br><sub>Slack webhook URL</sub> | `SLACK_WEBHOOK` | ✓ | 96 | `https://hooks.slack.com/services/T000/B000/XXXXXXXX` |
| `google_api_key` | Google API キー<br><sub>Google API key</sub> | `GOOGLE_API_KEY` | ✓ | 95 | `AIzaSyA-1234567890abcdefghijklmnopqrstu` |
| `google_oauth_secret` | Google OAuth クライアントシークレット<br><sub>Google OAuth client secret</sub> | `GOOGLE_SECRET` | ✓ | 95 | `GOCSPX-abcdefghijklmnopqrstuvwxyz12` |
| `azure_key` | Azure ストレージ等のキー<br><sub>Azure storage / service bus key</sub> | `AZURE_KEY` | ✓ | 95 | `AccountName=demo;AccountKey=abcdEFGHijklMNOPqrstUVWX0123456789==` |
| `stripe_key` | Stripe API キー<br><sub>Stripe API key</sub> | `STRIPE_KEY` | ✓ | 95 | `sk_test_abcdefghijklmnop1234` |
| `anthropic_key` | Anthropic API キー<br><sub>Anthropic API key</sub> | `API_KEY` | ✓ | 97 | `sk-ant-api03-abcdefghijklmnopqrstuvwxyz` |
| `openai_key` | OpenAI 形式の API キー (sk-...)<br><sub>OpenAI-style API key (sk-...)</sub> | `API_KEY` | ✓ | 95 | `sk-proj-abcdefghijklmnopqrstuvwxyz0123` |
| `huggingface_token` | Hugging Face トークン<br><sub>Hugging Face token</sub> | `HF_TOKEN` | ✓ | 95 | `hf_abcdefghijklmnopqrstuvwxyzABCDEF` |
| `npm_token` | npm トークン<br><sub>npm token</sub> | `NPM_TOKEN` | ✓ | 95 | `npm_abcdefghijklmnopqrstuvwxyz0123456789` |
| `sendgrid_key` | SendGrid API キー<br><sub>SendGrid API key</sub> | `SENDGRID_KEY` | ✓ | 95 | `SG.abcdefghijklmnopqrstuv.abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG` |
| `jwt` | JWT (JSON Web Token)<br><sub>JSON Web Token</sub> | `JWT` | ✓ | 93 | `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dummysignature123` |
| `bearer_token` | Bearer トークン<br><sub>Bearer token</sub> | `TOKEN` | ✓ | 92 | `Authorization: Bearer abcdefghijklmnop0123456789` |
| `basic_auth` | HTTP Basic 認証ヘッダー<br><sub>HTTP Basic auth header</sub> | `BASIC_AUTH` | ✓ | 92 | `Authorization: Basic dXNlcjpwYXNzd29yZA==` |
| `password_kv` | key=value 形式のパスワード・秘密値<br><sub>Password / secret in key=value</sub> | `PASSWORD` | ✓ | 90 | `db_password=Tr0ub4dor&3` |
| `password_ja` | パスワード (日本語の項目名)<br><sub>Password (Japanese label)</sub> | `PASSWORD` | ✓ | 90 | `初期パスワード: Abc12345` |
| `high_entropy` | 高エントロピー文字列 (秘密値の可能性)<br><sub>High-entropy string (possible secret)</sub> | `SECRET` |  | 20 | `key: Zx8Qp2Lm9Vt4Rw7Ky1Hs6Nb3` |

## 位置情報 (Location) — `location`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `lat_lon` | 緯度経度<br><sub>Latitude / longitude</sub> | `GEO` | ✓ | 60 | `位置: 35.681236, 139.767125` |
| `jp_plate` | 自動車のナンバー<br><sub>Vehicle license plate (Japan)</sub> | `PLATE` | ✓ | 60 | `品川 300 あ 12-34` |
| `place_name` | 地名 (辞書: 〜市・〜駅・〜在住 など)<br><sub>Place name (dictionary, e.g. 〜市/〜駅/〜在住)</sub> | `PLACE` |  | 38 | `新宿駅の近く` |

## 組織 (Organization) — `organization`

| ID | 名前 | ラベル | 既定 | 優先度 | 例 |
|---|---|---|:---:|---:|---|
| `company_jp` | 会社名・法人名<br><sub>Company name (Japanese)</sub> | `COMPANY` | ✓ | 42 | `株式会社サンプル商事 御中` |
| `company_en` | 会社名 (英語表記)<br><sub>Company name (English)</sub> | `COMPANY` | ✓ | 42 | `contract with Example Widgets Inc.` |

## カスタム — `custom`

設定ファイルの `[[custom_rules]]` (正規表現) と `[[keywords]]` (キーワード辞書) で追加します。ID はそれぞれ `custom:<id>`、`keyword:<label>` になります。
