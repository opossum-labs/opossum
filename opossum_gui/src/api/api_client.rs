use opossum_backend::{
    error::ErrorResponse,
    general::{NodeType, VersionInfo},
    nodes::{ConnectInfo, NodeInfo},
    AnalyzerType, NodeAttr,
};
use reqwest::{Client, Response};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
pub struct HTTPAPIClient {
    client: Client,
    base_url: String,
}

impl Default for HTTPAPIClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HTTPAPIClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:8001".to_string(),
        }
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

    pub async fn get<R: Serialize + DeserializeOwned>(&self, route: &str) -> Result<R, String> {
        let res = self.client().get(self.url(route)).send().await;
        if let Ok(response) = res {
            self.process_response::<R>(response).await
        } else {
            Err(format!("Error on get request from route: \"{route}\""))
        }
    }

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

    //General api calls
    pub async fn get_version(&self) -> Result<VersionInfo, String> {
        self.get::<VersionInfo>("/api/version").await
    }
    pub async fn get_node_types(&self) -> Result<Vec<NodeType>, String> {
        self.get::<Vec<NodeType>>("/api/node_types").await
    }
    pub async fn get_api_welcome(&self) -> Result<String, String> {
        self.get::<String>("/api/").await
    }

    //Scenery api calls
    pub async fn delete_scenery(&self) -> Result<String, String> {
        self.delete::<String, String>("/api/scenery/", String::new())
            .await
    }
    // pub async fn get_analyzers(&self) -> Result<Vec<AnalyzerType>, String> {
    //     self.get::<Vec<AnalyzerType>>("/api/scenery/analyzers")
    //         .await
    // }
    pub async fn post_add_analyzer(
        &self,
        analyzer: AnalyzerType,
    ) -> Result<Vec<AnalyzerType>, String> {
        self.post::<AnalyzerType, Vec<AnalyzerType>>("/api/scenery/analyzers", analyzer)
            .await
    }
    // pub async fn get_analyzer_at_index(&self, index: usize) -> Result<AnalyzerType, String> {
    //     self.get::<AnalyzerType>(&format!("/api/scenery/analyzers/{}", index))
    //         .await
    // }
    // pub async fn delete_analyzer_at_index(&self, index: usize) -> Result<String, String> {
    //     self.delete::<String>(&format!("/api/scenery/analyzers/{}", index), index)
    //         .await
    // }

    //Node api calls
    pub async fn get_nodes(&self) -> Result<Vec<NodeInfo>, String> {
        self.get::<Vec<NodeInfo>>("/api/scenery/nodes").await
    }
    pub async fn post_add_node(
        &self,
        node_type: String,
        group_id: Uuid,
    ) -> Result<NodeInfo, String> {
        self.post::<String, NodeInfo>(
            &format!("/api/scenery/{}/nodes", group_id.as_simple()),
            node_type,
        )
        .await
    }
    pub async fn delete_node(&self, id: Uuid) -> Result<Vec<Uuid>, String> {
        self.delete::<String, Vec<Uuid>>(
            &format!("/api/scenery/{}/nodes", id.as_simple()),
            String::new(),
        )
        .await
    }

    pub async fn get_node_properties(&self, uuid: Uuid) -> Result<NodeAttr, String> {
        self.get::<NodeAttr>(&format!("/api/scenery/{}/properties", uuid.as_simple()))
            .await
    }
    pub async fn post_add_connection(
        &self,
        connection: ConnectInfo,
    ) -> Result<ConnectInfo, String> {
        self.post::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
            .await
    }

    pub async fn delete_connection(&self, connection: ConnectInfo) -> Result<ConnectInfo, String> {
        self.delete::<ConnectInfo, ConnectInfo>("/api/scenery/connection", connection)
            .await
    }
    // pub async fn put_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
    //     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
    // }
    // pub async fn patch_node_properties(&self, uuid: Uuid, props: NodeAttr) -> Result<NodeAttr, String> {
    //     self.put::<NodeAttr, NodeAttr>(&format!("/api/scenery/nodes/{}", uuid.as_simple().to_string()), props.serialize(serializer)).await
    // }
}
