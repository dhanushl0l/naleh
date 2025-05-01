use std::{
    env,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use actix_files as afs;
use actix_multipart::Multipart;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, web};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use zip::read::ZipArchive;

const PASS: &str = "pass";

async fn serve_page(req: HttpRequest) -> impl Responder {
    let page = req.match_info().query("filename");

    let content = match page {
        "home" => &fs::read_to_string("static/home.html").unwrap(),
        "about" => &fs::read_to_string("static/about.html").unwrap(),
        "services" => &fs::read_to_string("static/shop.html").unwrap(),
        "admin" => &fs::read_to_string("static/admin.html").unwrap(),
        _ => "<h1>404 Not Found</h1><p>The page you are looking for does not exist.</p>",
    };

    let full_page = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>{}</title>
            <style>{}</style>
        </head>
        <body>
            <nav>
                <ul>
                    <li><a href="/home">Home</a></li>
                    <li><a href="/services">Services</a></li>
                    <li><a href="/about">About</a></li>
                </ul>
            </nav>
        {}
        </body>
        </html>
        "#,
        page.to_uppercase(),
        &fs::read_to_string("static/style.css").unwrap(),
        content
    );

    HttpResponse::Ok().content_type("text/html").body(full_page)
}

async fn list_files(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let pass = query.get("pass").unwrap_or(&"".to_string()).clone();

    let path = PathBuf::from("static/items");
    match fs::read_dir(&path) {
        Ok(entries) => {
            if pass == PASS {
                let files: Vec<String> = entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect();
                HttpResponse::Ok().json(files)
            } else {
                HttpResponse::InternalServerError().body("auth failed")
            }
        }
        Err(_) => HttpResponse::InternalServerError().body("Failed to read directory"),
    }
}

async fn delete_file(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let filename = path.into_inner();
    let file_path = PathBuf::from(format!("static/items/{}", filename));

    let pass = query.get("pass").unwrap_or(&"".to_string()).clone();

    if pass == PASS {
        if file_path.exists() {
            match fs::remove_file(&file_path) {
                Ok(_) => HttpResponse::Ok().body(format!("{} deleted successfully", filename)),
                Err(_) => HttpResponse::InternalServerError().body("Failed to delete file"),
            }
        } else {
            HttpResponse::NotFound().body("File not found")
        }
    } else {
        HttpResponse::NotFound().body("unauthorized access")
    }
}

async fn entry() -> impl Responder {
    HttpResponse::NotFound().body("unauthorized access")
}

#[derive(Debug, Serialize, Deserialize)]
struct FormData {
    pub firstname: String,
    description: String,
    price: f64,
    weight: f64,
    r#type: String,
    rank: i32,
    securitykey: String,
}

#[derive(Debug, Serialize)]
struct SanitizedFormData {
    firstname: String,
    description: String,
    price: f64,
    weight: f64,
    r#type: String,
    rank: i32,
}

const SECURITY_KEY: &str = "1";

async fn submit(mut payload: Multipart) -> impl Responder {
    println!("Received a request to /submit");

    let mut form_data = None;
    let mut images = vec![];

    while let Some(item) = payload.next().await {
        if let Ok(mut field) = item {
            if field.name().unwrap() == "data" {
                // Handle form data (JSON)
                let mut field_data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(data) = chunk {
                        field_data.extend_from_slice(&data);
                    }
                }
                let json_str = String::from_utf8(field_data).unwrap();
                form_data = Some(serde_json::from_str::<FormData>(&json_str).unwrap());
            } else if field.name().unwrap().starts_with("image") {
                // Handle file uploads (images)
                let mut file_data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(data) = chunk {
                        file_data.extend_from_slice(&data);
                    }
                }
                images.push(file_data);
            }
        }
    }

    if let Some(data) = form_data {
        let name = data.firstname.clone();
        let dir_path = format!("./{}/images", name);
        fs::create_dir_all(&dir_path).unwrap();

        for (i, image_data) in images.into_iter().enumerate() {
            let image_path = format!("{}/image{}.jpg", dir_path, i + 1);
            let mut image_file = File::create(&image_path).unwrap();
            image_file.write_all(&image_data).unwrap();
        }

        // Optionally, save JSON with sanitized data
        let sanitized_data = format!(
            "{{\"name\":\"{}\",\"description\":\"{}\",\"price\":{}}}",
            data.firstname, data.description, data.price
        );
        let json_path = format!("./{}/data.json", name);
        fs::write(json_path, sanitized_data).unwrap();

        return HttpResponse::Ok()
            .json(json!({ "message": "File uploaded and form processed successfully!" }));
    }

    HttpResponse::BadRequest().json(json!({ "message": "Invalid file upload or form data." }))
}

#[derive(Serialize)]
struct Product {
    name: String,
    price: String,
    old_price: String,
    banner: String,
    images: Vec<String>,
}

async fn get_products() -> impl Responder {
    let common_images = vec![
        "static/img/img1.jpg".into(),
        "static/img/img2.jpg".into(),
        "static/img/img3.jpg".into(),
    ];

    let products = vec![
        Product {
            name: "Product 1".into(),
            price: "₹26,999".into(),
            old_price: "₹29,999".into(),
            banner: "static/img/img1.jpg".into(),
            images: common_images.clone(),
        },
        Product {
            name: "Product 2".into(),
            price: "₹19,999".into(),
            old_price: "₹24,999".into(),
            banner: "static/img/img1.jpg".into(),
            images: common_images.clone(),
        },
    ];

    HttpResponse::Ok().json(products)
}

use env_logger::{self, Env};
use log::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    info!("Starting server on http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            .service(afs::Files::new("/static", "static").show_files_listing())
            .route("/files", web::get().to(list_files))
            .route("/delete/{filename}", web::delete().to(delete_file))
            .route("entry", web::get().to(entry))
            .route("/{filename}", web::get().to(serve_page))
            .route("/submite", web::post().to(submit))
            .route("/api/products", web::get().to(get_products))
    })
    .bind("0.0.0.0:8081")?
    .run()
    .await
}
