use opossum_core::types::api_types::ErrorResponse;
use reqwest::{Client, Response, header::ACCEPT};
use serde::{Serialize, de::DeserializeOwned};
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

    /// Send a POST request to the given route with no body.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails or if the response cannot be
    /// deserialized into the expected type.
    pub async fn post_no_body<R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
    ) -> Result<R, String> {
        let res = self.client().post(self.url(route)).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on post request on route: \"{route}\""))
        }
    }
    /// Send a PUT request with a raw string body, but expect a JSON response.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails or if the response cannot be deserialized into the expected type.
    pub async fn put_string_receive_json<R: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: String,
    ) -> Result<R, String> {
        // Use .body() to send the raw string exactly as it is, NOT .json()
        let res = self.client().put(self.url(route)).body(body).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on put request on route: \"{route}\""))
        }
    }
    /// Send a PUT request to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn put<B: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<(), String> {
        let res = self.client().put(self.url(route)).json(&body).send().await;
        Self::no_content_result(res, route, "put").await
    }

    /// Send a POST request to the given route with the provided body, expecting no content (204).
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails (e.g. the route is not reachable)
    /// or if the response status indicates an error (e.g. a rejected value).
    pub async fn post_receive_no_content<B: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<(), String> {
        let res = self.client().post(self.url(route)).json(&body).send().await;
        Self::no_content_result(res, route, "post").await
    }

    /// Turns the raw result of a request expecting an empty `204` body into `Result<(), String>`.
    ///
    /// Shared by [`Self::put_receive_no_content`] and [`Self::post_receive_no_content`], both of
    /// which used to treat *any* response that merely arrived (including a `4xx`/`5xx`) as success -
    /// silently swallowing server-side rejections such as a validation error. This checks the status
    /// the same way [`Self::delete_no_content`] already does, and additionally surfaces the server's
    /// [`ErrorResponse`] message when the body carries one.
    async fn no_content_result(
        res: reqwest::Result<Response>,
        route: &str,
        verb: &str,
    ) -> Result<(), String> {
        let response = res.map_err(|_| format!("Error on {verb} request on route: \"{route}\""))?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        match response.json::<ErrorResponse>().await {
            Ok(err_res) => Err(format!(
                "Error {}: {} - {}",
                err_res.status, err_res.category, err_res.message
            )),
            Err(_) => Err(format!(
                "{verb} request to \"{route}\" failed with status: {status}"
            )),
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
    pub async fn patch<B: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<(), String> {
        let res = self
            .client()
            .patch(self.url(route))
            .json(&body)
            .send()
            .await;
        res.map_or_else(
            |_| Err(format!("Error on patch request on route: \"{route}\"")),
            |_| Ok(()),
        )
    }

    pub async fn patch_ron<B: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<(), String> {
        if let Ok(serialized) = ron::ser::to_string(&body) {
            let res = self
                .client()
                .patch(self.url(route))
                .header("Content-Type", "application/ron")
                .body(serialized)
                .send()
                .await
                .map_err(|_| format!("Error on patch request on route: \"{route}\""))?;

            if res.status().is_success() {
                Ok(())
            } else {
                let status = res.status();
                let text = res
                    .text()
                    .await
                    .unwrap_or_else(|_| "<failed to read body>".into());
                Err(format!("HTTP {status} on \"{route}\": {text}"))
            }
        } else {
            Err("Error serializing body using ron".to_string())
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
    pub async fn delete<R: Serialize + DeserializeOwned>(&self, route: &str) -> Result<R, String> {
        let res = self.client().delete(self.url(route)).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on delete request from route: \"{route}\""))
        }
    }

    /// Send a DELETE request to the given route without a body, expecting no content (204).
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response status indicates an error
    pub async fn delete_no_content(&self, route: &str) -> Result<(), String> {
        // Send the request without a .json() body
        let res = self.client().delete(self.url(route)).send().await;

        res.map_or_else(
            |_| Err(format!("Error on delete request from route: \"{route}\"")),
            |response| {
                if response.status().is_success() {
                    // 204 No Content is a success status, so we return an empty Ok(())
                    Ok(())
                } else {
                    // If it fails, we try to parse your ErrorResponse struct as a fallback
                    // assuming you have an ErrorResponse similar to your process_response_ron method
                    Err(format!(
                        "Delete request failed with status: {}",
                        response.status()
                    ))
                }
            },
        )
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
    /// Helper method to format an error response.
    ///
    /// It first tries to deserialize the body into an `ErrorResponse`.
    /// If that fails (e.g. plain text 413, HTML error page), it falls back
    /// to formatting the HTTP status code and raw response body.
    async fn handle_error_response(res: Response) -> String {
        let status = res.status();
        let reason = status.canonical_reason().unwrap_or("Unknown Status");

        // Read the body as plain text first so we do not lose it on deserialization failure
        let body_text = res
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".to_string());

        // Attempt to parse the structured OPOSSUM ErrorResponse JSON
        if let Ok(err_res) = serde_json::from_str::<ErrorResponse>(&body_text) {
            format!(
                "Error {}: {} - {}",
                err_res.status, err_res.category, err_res.message
            )
        } else {
            let trimmed_body = body_text.trim();
            if trimmed_body.is_empty() {
                format!("HTTP request failed with status {status} ({reason})")
            } else {
                format!("HTTP request failed with status {status} ({reason}): {trimmed_body}")
            }
        }
    }
    /// Process the response from the server.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the response cannot be deserialized into the expected type
    /// - the server returned a non-success HTTP status code
    pub async fn process_response<R: Serialize + DeserializeOwned>(
        &self,
        res: Response,
    ) -> Result<R, String> {
        if res.status().is_success() {
            if res.content_length().unwrap_or(0) > 0 {
                res.json::<R>().await.map_err(|e| {
                    format!("Failed to deserialize response body into requested struct: {e}")
                })
            } else {
                let json_val = json!("");
                serde_json::from_value(json_val).map_err(|e| {
                    format!("Failed to deserialize default string when no content returned: {e}")
                })
            }
        } else {
            Err(Self::handle_error_response(res).await)
        }
    }
    /// Process the response of an API call expecting raw text.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request was not successful
    /// or if reading the response body fails.
    pub async fn process_response_raw(&self, res: Response) -> Result<String, String> {
        if res.status().is_success() {
            res.text().await.map_err(|e| e.to_string())
        } else {
            Err(Self::handle_error_response(res).await)
        }
    }
    /// Process the response from the server assuming RON format.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the server returned a non-success HTTP status code
    /// - the response cannot be deserialized from RON into the expected type
    pub async fn process_response_ron<R: Serialize + DeserializeOwned>(
        &self,
        res: Response,
    ) -> Result<R, String> {
        if res.status().is_success() {
            let text = res.text().await.map_err(|e| e.to_string())?;
            ron::from_str::<R>(&text).map_err(|e| format!("Failed to parse RON response: {e}"))
        } else {
            Err(Self::handle_error_response(res).await)
        }
    }
}
