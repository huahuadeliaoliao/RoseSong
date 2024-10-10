use crate::error::ApplicationError;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Owner {
    pub name: String,
}

#[derive(Deserialize)]
pub struct VideoData {
    pub bvid: String,
    pub title: String,
    pub cid: i64,
    pub owner: Owner,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

pub async fn fetch_video_data(client: &Client, bvid: &str) -> Result<VideoData, ApplicationError> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={bvid}"
    );
    let response = client.get(&url).send().await.map_err(|e| {
        eprintln!("Failed to send request to {url}: {e}");
        ApplicationError::HttpRequest(e)
    })?;
    let mut api_response: ApiResponse<VideoData> = response.json().await.map_err(|e| {
        eprintln!("Failed to parse response from {url}: {e}");
        ApplicationError::HttpRequest(e)
    })?;
    api_response.data.bvid = bvid.to_string();
    Ok(api_response.data)
}

pub async fn fetch_bvids_from_fid(
    client: &Client,
    fid: &str,
) -> Result<Vec<String>, ApplicationError> {
    let url = format!(
        "https://api.bilibili.com/x/v3/fav/resource/ids?media_id={fid}"
    );
    let response = client.get(&url).send().await.map_err(|e| {
        eprintln!("Failed to send request to {url}: {e}");
        ApplicationError::HttpRequest(e)
    })?;
    let json: serde_json::Value = response.json().await.map_err(|e| {
        eprintln!("Failed to parse response from {url}: {e}");
        ApplicationError::HttpRequest(e)
    })?;
    let bvids: Vec<String> = json["data"]
        .as_array()
        .ok_or_else(|| {
            eprintln!("Failed to find 'data' array in response from {url}");
            ApplicationError::DataParsingError("数据中缺少 bvids 数组".to_string())
        })?
        .iter()
        .filter_map(|v| v["bvid"].as_str().map(String::from))
        .collect();

    if bvids.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "提供的 fid 无效或没有找到相关的视频".to_string(),
        ));
    }

    Ok(bvids)
}

pub async fn get_video_data(
    client: &Client,
    fid: Option<&str>,
    bvid: Option<&str>,
) -> Result<Vec<VideoData>, ApplicationError> {
    let mut video_data_list = Vec::new();

    if let Some(fid) = fid {
        let bvids = fetch_bvids_from_fid(client, fid).await?;
        for bvid in bvids {
            let video_data = fetch_video_data(client, &bvid).await?;
            video_data_list.push(video_data);
        }
    } else if let Some(bvid) = bvid {
        let video_data = fetch_video_data(client, bvid).await?;
        video_data_list.push(video_data);
    } else {
        return Err(ApplicationError::InvalidInput(
            "请提供正确的 fid 或 bvid".to_string(),
        ));
    }

    if video_data_list.is_empty() {
        return Err(ApplicationError::InvalidInput(
            "提供的 fid 或 bvid 无效或没有找到相关的视频".to_string(),
        ));
    }

    Ok(video_data_list)
}
