use opossum_backend::error::ErrorResponse;
use reqwest::{header::ACCEPT, Client, Response};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

#[derive(Clone)]
pub struct HTTPClient {
    client: Client,
    base_url: String,
}

impl Default for HTTPClient {
    fn default() -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:8001".to_string(),
        }
    }
}

impl HTTPClient {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub const fn client(&self) -> &Client {
        &self.client
    }
    #[must_use]
    pub const fn base_url(&self) -> &String {
        &self.base_url
    }
    #[must_use]
    pub fn url(&self, route: &str) -> String {
        format!("{}{}", self.base_url, route)
    }
    /// Send a POST reqeust to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails or if the response cannot be deserialized into the expected type.
    pub async fn post<B: Serialize + DeserializeOwned + Clone, R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<R, String> {
        let res = self.client().post(self.url(route)).json(&body).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on post request on route: \"{route}\""))
        }
    }
    /// Send a POST reqeust to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails or if the response cannot be deserialized into the expected type.
    pub async fn post_string(&self, route: &str, body: String) -> Result<String, String> {
        let res = self.client().post(self.url(route)).body(body).send().await;
        if let Ok(response) = res {
            self.process_response::<String>(response).await
        } else {
            Err(format!("Error on post request on route: \"{route}\""))
        }
    }
    /// Send a PUT request to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn put<B: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<R, String> {
        let res = self.client().put(self.url(route)).json(&body).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on put request on route: \"{route}\""))
        }
    }

    /// Send a PATCH request to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn patch<B: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<R, String> {
        let res = self
            .client()
            .patch(self.url(route))
            .json(&body)
            .send()
            .await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on patch request on route: \"{route}\""))
        }
    }

    /// Send a DELETE request to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn delete<B: Serialize + DeserializeOwned, R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<R, String> {
        let res = self
            .client()
            .delete(self.url(route))
            .json(&body)
            .send()
            .await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on delete request from route: \"{route}\""))
        }
    }

    /// Send a GET request to the given route.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn get<R: Serialize + DeserializeOwned>(&self, route: &str) -> Result<R, String> {
        let res = self.client().get(self.url(route)).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on get request from route: \"{route}\""))
        }
    }
    /// Send a GET request to the given route and expect a pure `string`.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into a `string`
    pub async fn get_raw(&self, route: &str) -> Result<String, String> {
        let res = self.client().get(self.url(route)).send().await;
        if let Ok(response) = res {
            self.process_response_raw(response).await
        } else {
            Err(format!("Error on get request from route: \"{route}\""))
        }
    }

    /// Send a GET request to the given route accepting RON data
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn get_ron<R: Serialize + DeserializeOwned>(&self, route: &str) -> Result<R, String> {
        let res = self
            .client()
            .get(self.url(route))
            .header(ACCEPT, "application/ron")
            .send()
            .await;
        if let Ok(response) = res {
            self.process_response_ron::<R>(response).await
        } else {
            Err(format!("Error on get request from route: \"{route}\""))
        }
    }

    /// Process the response from the server.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the response cannot be deserialized into the expected type
    pub async fn process_response<R: Serialize + DeserializeOwned>(
        &self,
        res: Response,
    ) -> Result<R, String> {
        if res.status().is_success() {
            if res.content_length().map_or_else(|| 0, |n| n) > 0 {
                (res.json::<R>().await).map_or_else(
                    |_| Err("Error deserializing response to requested struct!".to_string()),
                    |res| Ok(res),
                )
            } else {
                // just to receive a value i nothing has been sent back
                let json_val = json!("");
                serde_json::from_value(json_val).map_or_else(|_| Err("Error deserializing default string if no content returns!".to_string()), |deserialized| Ok(deserialized))
            }
        } else if let Ok(err_res) = res.json::<ErrorResponse>().await {
            Err(format!(
                "Error {}: {} - {}",
                err_res.status(),
                err_res.category(),
                err_res.message()
            ))
        } else {
            Err("Error deserializing response to ErrorResponse struct!".to_string())
        }
    }
    /// Process the response of an API call.
    ///
    /// This a special version of the more general `process_response` function which handles pure `string` responses.
    /// This function is used for handling the generation of an `OPM` file string.
    ///
    /// # Panics
    ///
    /// Panics if the returned data cannot be parsed as text.
    ///
    /// # Errors
    ///
    /// This function will return an error if the response .
    pub async fn process_response_raw(&self, res: Response) -> Result<String, String> {
        if res.status().is_success() {
            Ok(res.text().await.unwrap())
        } else {
            Err("Error deserializing response to ErrorResponse struct!".to_string())
        }
    }
    /// Process the response from the server assuming RON format
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the response cannot be deserialized into the expected type
    ///
    /// # Panics
    ///
    /// Panics if the returned data cannot be parsed as text (before parsed fon RON into the final data type).
    pub async fn process_response_ron<R: Serialize + DeserializeOwned>(
        &self,
        res: Response,
    ) -> Result<R, String> {
        if res.status().is_success() {
            let text = res.text().await.unwrap();
            let data: R =
                ron::from_str(&text).map_err(|e| format!("parsing of data failed: {e}"))?;
            Ok(data)
        } else {
            Err("Error deserializing response to ErrorResponse struct!".to_string())
        }
    }
}
