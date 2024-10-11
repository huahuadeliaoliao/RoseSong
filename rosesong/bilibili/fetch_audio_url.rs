use crate::error::App;
use reqwest::Client;
use serde_json::Value;

const BASE_API_URL: &str = "https://api.bilibili.com/x/player/playurl?fnval=16";

pub async fn fetch_audio_url(client: &Client, bvid: &str, cid: &str) -> Result<String, App> {
    let url = format!("{BASE_API_URL}&bvid={bvid}&cid={cid}");
    log::info!("Fetching audio URL");
    let response = client.get(&url).send().await?;
    let json: Value = response.json().await?;
    json["data"]["dash"]["audio"][0]["baseUrl"]
        .as_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| App::DataParsing("解析音频URL失败".to_string()))
}
