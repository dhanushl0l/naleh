use std::{
    env,
    fs::{self, File, create_dir_all},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use actix_files as afs;
use actix_multipart::Multipart;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, web};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

const PASS: Option<&str> = option_env!("PASS");

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
        &fs::read_to_string("static/colors.css").unwrap(),
        content
    );

    HttpResponse::Ok().content_type("text/html").body(full_page)
}

async fn list_files(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let pass = query.get("pass").unwrap_or(&"".to_string()).clone();

    let path = PathBuf::from(DATABASE_PATH);
    match fs::read_dir(&path) {
        Ok(entries) => {
            println!("{}||{}", pass, PASS.unwrap());
            if pass == PASS.unwrap() {
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

    if pass == PASS.unwrap() {
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
struct SanitizedFormData {
    firstname: String,
    description: String,
    price: i32,
    weight: i32,
    typee: String,
    rank: i32,
    url: String,
}

const DATABASE_PATH: &str = "database";

async fn submit(
    mut payload: Multipart,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let key = query.get("key").cloned().unwrap_or_default();

    if &key == PASS.unwrap() {
        while let Some(item) = payload.next().await {
            let mut field = match item {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Multipart error: {:?}", e);
                    return HttpResponse::BadRequest().body("Invalid multipart data");
                }
            };

            let content_disposition = field.content_disposition();
            let field_name = content_disposition.unwrap().get_name().unwrap_or_default();
            if field_name != "file" {
                continue;
            }

            let mut data = web::BytesMut::new();
            while let Some(chunk) = field.next().await {
                data.extend_from_slice(&chunk.unwrap());
            }

            let cursor = Cursor::new(data);
            let mut zip = ZipArchive::new(cursor).unwrap();

            let mut data: Option<SanitizedFormData> = None;
            let mut images = Vec::new();
            for i in 0..zip.len() {
                let mut file = zip.by_index(i).unwrap();
                let name = file.name().to_string();

                if name.ends_with('/') {
                    continue;
                }

                if name == "data.json" {
                    let mut json_string = String::new();
                    file.read_to_string(&mut json_string).unwrap();
                    data = serde_json::from_str(&json_string).unwrap();
                    println!("{:?}", data);
                } else if name.starts_with("images/") {
                    let filename = name
                        .strip_prefix("images/")
                        .unwrap_or("image.png")
                        .to_string();
                    let mut buffer = Vec::new();
                    file.read_to_end(&mut buffer).unwrap();
                    images.push((filename, buffer));
                }
            }

            if let Some(data) = data {
                let mut path = PathBuf::from(DATABASE_PATH);
                path.push(&data.firstname);
                std::fs::create_dir_all(&path).unwrap();

                path.push("user.json");
                let json = serde_json::to_string_pretty(&data).unwrap();
                fs::write(&path, json).unwrap();
                path.pop();
                path.push("image");
                fs::create_dir_all(&path).unwrap();
                for (name, data) in images {
                    path.push(&name);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    let mut file = File::create(&path).unwrap();
                    file.write_all(&data).unwrap();
                    path.pop();
                }
            } else {
                return HttpResponse::InternalServerError().body("Error parsing json");
            }
        }
    } else {
        return HttpResponse::Unauthorized().body("invalid password");
    }

    HttpResponse::Ok().body("Submission received and processed")
}

#[derive(Debug, Serialize, Deserialize)]
struct Prodects {
    firstname: String,
    description: String,
    price: i32,
    weight: i32,
    typee: String,
    rank: i32,
    url: String,
    images: Vec<String>,
}

impl Prodects {
    fn from(data: SanitizedFormData, images: Vec<String>) -> Self {
        Self {
            firstname: data.firstname,
            description: data.description,
            price: data.price,
            weight: data.weight,
            typee: data.typee,
            rank: data.rank,
            url: data.url,
            images,
        }
    }
}
async fn get_products() -> impl Responder {
    let mut products = Vec::new();

    if let Ok(entries) = fs::read_dir(DATABASE_PATH) {
        for entry in entries {
            if let Ok(entry) = entry {
                let mut path = entry.path();
                path.push("user.json");
                let file = fs::read_to_string(&path).unwrap();
                let data: SanitizedFormData = serde_json::from_str(&file).unwrap();
                let mut prod_images = Vec::new();
                path.pop();
                path.push("image");
                if let Ok(images) = fs::read_dir(path) {
                    for image in images {
                        if let Ok(image) = image {
                            let image: String = image.path().to_string_lossy().into_owned();
                            prod_images.push(image);
                        }
                    }
                }
                let data = Prodects::from(data, prod_images);
                products.push(data);
            }
        }
    }

    HttpResponse::Ok().json(products)
}

use env_logger::{self, Env};
use log::info;
use zip::ZipArchive;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().filter_or("LOG", "info")).init();

    info!("Starting server on http://127.0.0.1:8081");

    HttpServer::new(|| {
        App::new()
            .service(afs::Files::new("/database", "./database").show_files_listing())
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
