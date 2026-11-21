//! Cliente bloqueante de la API web de RetroAchievements.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::model::{GameEntry, GameHash};

/// URL del repositorio para el User-Agent; sustituir `<owner>` cuando exista el repo en GitHub.
pub const REPO_URL: &str = "https://github.com/<owner>/ra-hash-checker";

/// Los mensajes de `Network`/`Parse` solo se usan por `Debug`; la UI muestra un código localizado.
#[allow(dead_code)]
#[derive(Debug)]
pub enum RaError {
    Unauthorized,
    Network(String),
    Parse(String),
}

pub struct RaClient {
    http: reqwest::blocking::Client,
    base_url: String,
    api_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HashesResponse {
    results: Vec<GameHash>,
}

/// La URL lleva la clave (`y=`): se quita antes de convertir el error en texto.
fn network_error(e: reqwest::Error) -> RaError {
    RaError::Network(e.without_url().to_string())
}

impl RaClient {
    pub fn new(base_url: &str, api_key: String) -> RaClient {
        let user_agent = format!("RAHashChecker/{} (+{})", env!("CARGO_PKG_VERSION"), REPO_URL);
        let http = reqwest::blocking::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("cliente HTTP");
        RaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub fn get_game_list(&self, console_id: u32) -> Result<Vec<GameEntry>, RaError> {
        let url = format!("{}/API_GetGameList.php", self.base_url);
        self.get_json(&url, &[("i", console_id.to_string()), ("h", "1".into())])
    }

    pub fn get_game_hashes(&self, game_id: u32) -> Result<Vec<GameHash>, RaError> {
        let url = format!("{}/API_GetGameHashes.php", self.base_url);
        let resp: HashesResponse = self.get_json(&url, &[("i", game_id.to_string())])?;
        Ok(resp.results)
    }

    fn get_json<T: DeserializeOwned>(&self, url: &str, params: &[(&str, String)]) -> Result<T, RaError> {
        let resp = self
            .http
            .get(url)
            .query(params)
            .query(&[("y", self.api_key.as_str())])
            .send()
            .map_err(network_error)?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RaError::Unauthorized);
        }
        if !status.is_success() {
            return Err(RaError::Network(format!("HTTP {status}")));
        }
        let body = resp.text().map_err(network_error)?;
        serde_json::from_str(&body).map_err(|e| RaError::Parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    #[test]
    fn game_list_parses_and_sends_params() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("i".into(), "5".into()),
                Matcher::UrlEncoded("h".into(), "1".into()),
                Matcher::UrlEncoded("y".into(), "KEY".into()),
            ]))
            .match_header("user-agent", Matcher::Regex("^RAHashChecker/".into()))
            .with_body(r#"[{"ID":1,"Title":"T","ConsoleID":5,"NumAchievements":3,"Hashes":["ABC"]}]"#)
            .create();
        let client = RaClient::new(&server.url(), "KEY".into());
        let list = client.get_game_list(5).unwrap();
        m.assert();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 1);
        assert_eq!(list[0].hashes, vec!["ABC".to_string()]);
    }

    #[test]
    fn game_list_query_has_no_f_param() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(Matcher::Exact("i=5&h=1&y=KEY".into()))
            .with_body("[]")
            .create();
        let client = RaClient::new(&server.url(), "KEY".into());
        assert!(client.get_game_list(5).unwrap().is_empty());
        m.assert();
    }

    #[test]
    fn game_hashes_parses_results() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/API_GetGameHashes.php")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("i".into(), "9".into()),
                Matcher::UrlEncoded("y".into(), "KEY".into()),
            ]))
            .with_body(r#"{"Results":[{"MD5":"abc","Name":"T (USA)","Labels":["nointro"],"PatchUrl":null}]}"#)
            .create();
        let client = RaClient::new(&server.url(), "KEY".into());
        let hashes = client.get_game_hashes(9).unwrap();
        assert_eq!(hashes[0].name, "T (USA)");
        assert_eq!(hashes[0].md5, "abc");
    }

    #[test]
    fn unauthorized_maps_to_error() {
        let mut server = mockito::Server::new();
        let _m = server.mock("GET", Matcher::Any).with_status(401).create();
        let client = RaClient::new(&server.url(), "BAD".into());
        assert!(matches!(client.get_game_list(5), Err(RaError::Unauthorized)));
    }

    #[test]
    fn network_error_does_not_contain_the_key() {
        let client = RaClient::new("http://127.0.0.1:1", "SECRETKEY123".into());
        for err in [client.get_game_list(5).unwrap_err(), client.get_game_hashes(9).unwrap_err()] {
            let RaError::Network(msg) = &err else { panic!("{err:?}") };
            assert!(!msg.is_empty());
            assert!(!format!("{err:?}").contains("SECRETKEY123"), "{err:?}");
        }
    }

    #[test]
    fn garbage_body_is_parse_error() {
        let mut server = mockito::Server::new();
        let _m = server.mock("GET", Matcher::Any).with_body("<html>").create();
        let client = RaClient::new(&server.url(), "KEY".into());
        assert!(matches!(client.get_game_list(5), Err(RaError::Parse(_))));
    }
}
