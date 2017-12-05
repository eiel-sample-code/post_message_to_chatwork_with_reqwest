// 使用する crate を宣言
extern crate reqwest; // シンプルなHTTPクライアント
extern crate serde;  // シリアライズライブラリ
extern crate serde_json; // serdeでJSONを扱うライブラリ
extern crate url;

// `#[derive(Serialize, Deserialize)`を使えるようにする
#[macro_use]
extern crate serde_derive;

// HTTPライブラリのデファクトスタンダード
// header! を使うだけ
#[macro_use]
extern crate hyper;

// reqwest::Url を Url と書けるようになります
use reqwest::Url;

use hyper::header::Headers;
// HTTPヘッダー用の構造体を生成してくれる
header! { (XChatWorkToken, "X-ChatWorkToken") => [String] }

/// HTTPリクエスト用とのマッピング用の構造体
// POSTパラメータにあわせて、構造体定義しておけば勝手にいい感じにしてくれる。便利
#[derive(Serialize)]
struct PostMessageRequest {
    body: String, // Bodyパラメータを設定する
}

/// メッセージ投稿APIのレスポンスとのマッピング用の構造体
// 帰ってくるJSONにあわせて構造体定義しておけば勝手にいい感じにしてくれる。便利
// #[serde(untagged)] でどのようにマッピングするか指定します。
// #[derive](Debug)]を書いておくとDebug出力を自動生成してくれます。
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum PostMessageResponse {
    Error { errors: Vec<String> },
    MessageId { message_id: String },
}

/// PostMessageResponseのままだと使いにくいので用意
#[derive(Debug)]
struct MessageId {
    message_id: String,
}

/// post_message関数で発生するエラーを一つの型にするためのenum
// 型を合わせる必要があるため作成、文字列にしてしまう手もある
#[derive(Debug)]
enum PostMessageError {
    Reqwest(reqwest::Error),
    UrlParse(url::ParseError),
    API(Vec<String>),
}

/// post_message関数でreqwest::Errorを返す関数を呼ぶときに勝手に変換できるようにする
// PostMessageErrorにFromトレイトを実装している
impl From<reqwest::Error> for PostMessageError {
    fn from(e: reqwest::Error) -> PostMessageError {
        PostMessageError::Reqwest(e)
    }
}

/// post_message関数でurl::ParserErrorを返す関数を呼ぶときに勝手に変換できるようにする
// PostMessageErrorにFromトレイトを実装している
impl From<url::ParseError> for PostMessageError {
    fn from(e: url::ParseError) -> PostMessageError {
        PostMessageError::UrlParse(e)
    }
}

/// みんなだいすきエントリーポイント
fn main() {
    // unwrap すると Result<A,B>な型のとき Aがかえってくる Bの値をもってるときはpanicがおきる
    // ResultはいわゆるEither型
    // `left` `right`ではなく `Ok` `Err`
    // 自分が使うツールぐらいだったら Resultな型はmain関数でunwrap
    let (room_id, body) = parse_args().unwrap();
    // tokenは何度か使いたいはずなので、 &str で使う
    let token = env_chatwork_token().unwrap();
    let response = post_message(&token, room_id, &body).unwrap();
    // {:?} を使うとデバッグ形式で出力できます
    println!("{:?}", response);
}

/// 環境変数 CHATWORK_API_TOKENから値を取り出す
fn env_chatwork_token() -> Result<std::string::String, String> {
    std::env::var("CHATWORK_API_TOKEN")
        // そのままのだとエラーの原因がよくわからないエラーメッセージを作成
        // 文字列は&strなので Stringに変換。
        // &'static str のままでもいい気はするけど今回はStringにしています
        .map_err(|_| "CHATWORK_API_TOKEN environment variable not present".to_string())
}

/// コマンドライン引数を解析する
fn parse_args() -> Result<(u32, String), String> {
    // コマンドライン引数の取得
    let mut args = std::env::args();
    args.next(); // プログラムの名前なので無視します
    let room_id = match args.next() {
        Some(s) => s.parse::<u32>()
            // u32は unsigned 32bit 整数。 or で失敗したときの値を作成
            // `?`を利用するとResult型の失敗している値の場合は、そのまま`return`
            // 成功している場合はResultの中から値を取り出せる
            .or(Err("arg1 expected number for room_id"))?,
        // そもそも 最初の引数が取得できなかった場合の値を作成
        // Resultを扱ってないので、 `?`を使わず自分で `return`
        None => return Err("arg1 expected room_id, found None".to_string()),
    };

    let body = match args.next() {
        Some(s) => s,
        // 二番目の引数を取得できなかったときの値を作成
        None => return Err("args2 expected body, found None".to_string()),
    };
    // Resultを返さないといけないのでOkで包む
    // Rustでは最後の式が戻り値に。
    // セミコロンを付けると() 型になってしまうので書かない
    Ok((room_id, body))
}

/// POSTするURLを作成する
fn post_message_url(room_id: u32) -> Result<Url, url::ParseError> {
    let url_str = format!("https://api.chatwork.com/v2/rooms/{}/messages", room_id);
    Url::parse(&url_str) // 文字列をURLに変換するのは失敗することがある。
}

/// アクセストークンをセットしたHTTPヘッダーを作成する
// Stringでなくて &strにしないと関数の引数に使った変数の所有権が移動してしまって使えなくなってしまう
// tokenは何度が使いまわしたいと想像がつくので、 &str にして貸すだけにしてあげてます
// (結局to_stringメソッドでクローンが生成されるのであまり意味はない)
fn chatwork_api_headers(token: &str) -> Headers {
    // headers.setは () を返すので、ワンラインではかけず…
    // setを使うので mutに
    let mut headers = Headers::new();
    headers.set(XChatWorkToken(token.to_string()));
    headers
}

/// HTTPリクエストをしてREST APIを実行してJSONに
/// Tに使える型 JSONに使える型を制限をかけているだけ
// UrlやHeaderは使いまわしたいかもしれませんが、利用しているライブラリの都合所有権を移動させてしまいます。
fn request_chatwork_api<T: serde::Serialize, JSON: serde::de::DeserializeOwned>
    (url: Url,
     headers: Headers,
     body: &T)
     -> Result<JSON, reqwest::Error> {
    reqwest::Client::new()
        .post(url)
        .form(body)
        .headers(headers)
        .send()? // HTTPリクエスト (Resultが返ってくる)
        .json() // JSONに変換
}

/// request_chatwork_api をラップして使いやすく
// u32はコピーされるので関数に渡しても、その後も使いまわせます(Copyトレイトが実装されているため)
// 型の不一致がおきてしまうので、まとめてあつかえるPostMessageErrorを用意
// 静的ディスパッチでなくなってもよいなら Box<std::error::Error>を使う手もたぶんある
fn post_message(token: &str, room_id: u32, body: &str) -> Result<MessageId, PostMessageError> {
    let body = PostMessageRequest { body: body.to_owned() };
    // Err は url::ParseError ですが Fromトレイトを実装しているので、PostMessageErrorに変換してくれます
    let url = post_message_url(room_id)?;
    let headers = chatwork_api_headers(token);
    let response = request_chatwork_api(url, headers, &body)?;
    // 使いやすいように値を変換して返す
    match response {
        PostMessageResponse::Error { errors } => Err(PostMessageError::API(errors)),
        PostMessageResponse::MessageId { message_id } => Ok(MessageId { message_id: message_id }),
    }
}
