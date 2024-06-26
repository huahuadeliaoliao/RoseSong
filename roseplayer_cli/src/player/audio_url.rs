use crate::error::ApplicationError;
use reqwest::Client;
use serde_json::Value;

const BASE_API_URL: &str = "https://api.bilibili.com/x/player/playurl?fnval=16";

pub async fn fetch_audio_url(
    client: &Client,
    bvid: &str,
    cid: &str,
) -> Result<String, ApplicationError> {
    let url = format!("{}&bvid={}&cid={}", BASE_API_URL, bvid, cid);
    println!("{url}");
    let response = client.get(&url).send().await?;
    let json: Value = response.json().await?;
    json["data"]["dash"]["audio"][0]["baseUrl"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ApplicationError::DataParsingError("解析音频URL失败".to_string()))
}
