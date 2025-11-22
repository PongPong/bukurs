use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::error::Error;

#[derive(Debug)]
pub struct FetchResult {
    pub url: String,
    pub title: String,
    pub desc: String,
    pub keywords: String,
}

pub fn fetch_data(url: &str) -> Result<FetchResult, Box<dyn Error>> {
    let client = Client::new();
    let resp = client.get(url).send()?;
    let final_url = resp.url().to_string();
    let body = resp.text()?;

    let document = Html::parse_document(&body);
    let title_selector = Selector::parse("title").unwrap();
    let meta_desc_selector = Selector::parse("meta[name='description']").unwrap();
    let meta_keywords_selector = Selector::parse("meta[name='keywords']").unwrap();

    let title = document
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<Vec<_>>().join(""))
        .unwrap_or_default();

    let desc = document
        .select(&meta_desc_selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .unwrap_or_default()
        .to_string();

    let keywords = document
        .select(&meta_keywords_selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .unwrap_or_default()
        .to_string();

    Ok(FetchResult {
        url: final_url,
        title,
        desc,
        keywords,
    })
}
