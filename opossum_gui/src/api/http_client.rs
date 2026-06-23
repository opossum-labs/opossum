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

    /// Send a POST request to the given route using RON data
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be serialized into the expected type
    pub async fn post_ron<
        B: Serialize + DeserializeOwned + Clone,
        R: Serialize + DeserializeOwned,
    >(
        &self,
        route: &str,
        body: B,
    ) -> Result<R, String> {
        if let Ok(serialized) = ron::ser::to_string(&body) {
            let res = self
                .client()
                .post(self.url(route))
                .header("Content-Type", "application/ron")
                .body(serialized)
                .send()
                .await;
            if let Ok(response) = res {
                self.process_response_ron::<R>(response).await
            } else {
                Err(format!("Error on post request on route: \"{route}\""))
            }
        } else {
            Err("Error serializing body using ron".to_string())
        }
    }

    /// Send a POST reqeust to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if the request fails or if the response cannot be deserialized into the expected type.
    pub async fn put_string(&self, route: &str, body: String) -> Result<String, String> {
        let res = self.client().put(self.url(route)).body(body).send().await;
        if let Ok(response) = res {
            self.process_response::<String>(response).await
        } else {
            Err(format!("Error on put request on route: \"{route}\""))
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
    /// Send a PUT request to the given route with the provided body.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the request fails (e.g. the route is not reachable)
    /// - the response cannot be deserialized into the expected type
    pub async fn put_receive_no_content<B: Serialize + DeserializeOwned>(
        &self,
        route: &str,
        body: B,
    ) -> Result<(), String> {
        let res = self.client().put(self.url(route)).json(&body).send().await;
        res.map_or_else(
            |_| Err(format!("Error on put request on route: \"{route}\"")),
            |_response| Ok(()),
        )
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
            if res.content_length().unwrap_or(0) > 0 {
                (res.json::<R>().await).map_or_else(
                    |_| Err("Error deserializing response to requested struct!".to_string()),
                    |res| Ok(res),
                )
            } else {
                // just to receive a value i nothing has been sent back
                let json_val = json!("");
                serde_json::from_value(json_val).map_or_else(|_| Err("Error deserializing default string if no content returns!".to_string()), |deserialized| Ok(deserialized))
            }
        } else {
            (res.json::<ErrorResponse>().await).map_or_else(
                |_| Err("Error deserializing response to ErrorResponse struct!".to_string()),
                |err_res| {
                    Err(format!(
                        "Error {}: {} - {}",
                        err_res.status, err_res.category, err_res.message
                    ))
                },
            )
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
            Ok(res.text().await.map_err(|e| e.to_string())?)
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
            let text = res.text().await.map_err(|e| e.to_string())?;
            let data: R =
                ron::from_str(&text).map_err(|e| format!("parsing of data failed: {e}"))?;
            Ok(data)
        } else {
            (res.json::<ErrorResponse>().await).map_or_else(
                |_| Err("Error deserializing response to ErrorResponse struct!".to_string()),
                |err_res| {
                    Err(format!(
                        "Error {}: {} - {}",
                        err_res.status, err_res.category, err_res.message
                    ))
                },
            )
        }
    }
}
