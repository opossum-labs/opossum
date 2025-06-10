use actix_web::{Responder, get, web::Html};
use utoipa_actix_web::service_config::ServiceConfig;

/// Return a welcome message
///
/// Return a static page with a welcome message.
#[utoipa::path(get, path = "/")]
#[get("/")]
async fn welcome() -> impl Responder {
    Html::new("<!DOCTYPE html>
<html lang=\"en\">
<head>
    <meta charset=\"UTF-8\">
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">
    <title>OPOSSUM Backend Server</title>
    <style>
        body {font-family: Arial, sans-serif;margin: 0;padding: 0;background-color: #f4f4f4;text-align: center;}
        .container {
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
            background: white;
            box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
            border-radius: 10px;
        }
        h1 {color: #333;}
        p {color: #666; font-size: 18px;}
        .link {
            display: inline-block;
            margin-top: 20px;
            padding: 10px 20px;
            background: #007BFF;
            color: white;
            text-decoration: none;
            border-radius: 5px;
            font-size: 18px;
        }
        .link:hover {background: #0056b3;}
    </style>
</head>
<body>
    <div class=\"container\">
        <h1>Welcome to the OPOSSUM Backend Server</h1>
        <p>The OPOSSUM backend server provides a robust and scalable API for communicating with the OPOSSUM library.</p>
        <a class=\"link\" href=\"https://git.gsi.de/phelix/rust/opossum\">OPOSSUM repository</a><br/>
        <a class=\"link\" href=\"swagger-ui/\">View API Documentation</a>
    </div>
</body>
</html>")
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(welcome);
}
#[cfg(test)]
mod test {
    use actix_web::{App, dev::Service, http::StatusCode, test};

    #[actix_web::test]
    async fn welcome() {
        let app = test::init_service(App::new().service(super::welcome)).await;
        let req = test::TestRequest::with_uri("/").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
