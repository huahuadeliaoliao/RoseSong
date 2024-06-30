use crate::error::ApplicationError;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Owner {
    name: String,
}

#[derive(Deserialize)]
struct VideoData {
    title: String,
    cid: i64,
    owner: Owner,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

async fn fetch_video_data(client: &Client, bvid: &str) -> Result<VideoData, ApplicationError> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let response = client.get(&url).send().await?;
    let api_response: ApiResponse<VideoData> = response.json().await?;
    Ok(api_response.data)
}

async fn fetch_bvids_from_media_id(
    client: &Client,
    media_id: &str,
) -> Result<Vec<String>, ApplicationError> {
    let url = format!(
        "https://api.bilibili.com/x/v3/fav/resource/ids?media_id={}",
        media_id
    );
    let response = client.get(&url).send().await?;
    let json: serde_json::Value = response.json().await?;
    let bvids = json["data"]
        .as_array()
        .ok_or_else(|| ApplicationError::DataParsingError("数据中缺少 bvids 数组".to_string()))?
        .iter()
        .filter_map(|v| v["bvid"].as_str().map(String::from))
        .collect();
    Ok(bvids)
}

async fn get_video_data(
    client: &Client,
    media_id: Option<&str>,
    bvid: Option<&str>,
) -> Result<Vec<VideoData>, ApplicationError> {
    let mut video_data_list = Vec::new();

    if let Some(media_id) = media_id {
        let bvids = fetch_bvids_from_media_id(client, media_id).await?;
        for bvid in bvids {
            let video_data = fetch_video_data(client, &bvid).await?;
            video_data_list.push(video_data);
        }
    } else if let Some(bvid) = bvid {
        let video_data = fetch_video_data(client, bvid).await?;
        video_data_list.push(video_data);
    } else {
        return Err(ApplicationError::DataParsingError(
            "请提供 media_id 或 bvid".to_string(),
        ));
    }

    Ok(video_data_list)
}
