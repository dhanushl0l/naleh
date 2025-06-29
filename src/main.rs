use actix_files as afs;
use actix_multipart::{Multipart, form::json};
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, web};
use futures_util::StreamExt;
use image::{ImageReader, imageops::FilterType};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::PathBuf,
};
use uuid::Uuid;

const PASS: Option<&str> = option_env!("PASS");

async fn serve_page(req: HttpRequest) -> impl Responder {
    let page = req.match_info().query("filename");

    let content = match page {
        "home" => &fs::read_to_string("static/home.html").unwrap(),
        "about" => &fs::read_to_string("static/about.html").unwrap(),
        "services" => &fs::read_to_string("static/shop.html").unwrap(),
        "admin" => &fs::read_to_string("static/admin.html").unwrap(),
        "shipment" => &fs::read_to_string("static/shipment.html").unwrap(),
        "return" => &fs::read_to_string("static/return.html").unwrap(),
        "" => &fs::read_to_string("static/home.html").unwrap(),
        _ => "<h1>404 Not Found</h1><p>The page you are looking for does not exist.</p>",
    };

    let full_page = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1.0" />
            <link rel="icon" type="image/png" href="static/img/ico.ico" />
            <title>nale cosmetics - affordable natural care {} page</title>

            <!-- SEO Meta Tags -->
            <meta name="description" content="Nale Cosmetics, founded in March 2025 in Tamil Nadu, India, offers high-quality and affordable personal care products that enhance your natural beauty." />
            <meta name="keywords" content="Nale, Nale Cosmetics, skincare, personal care, natural beauty, cosmetics India, Tamil Nadu beauty products" />
            <meta name="author" content="Nale Cosmetics" />
            <meta name="robots" content="index, follow" />
            <meta name="language" content="en" />
            <meta name="revisit-after" content="7 days" />

            <!-- Open Graph for social sharing -->
            <meta property="og:title" content="Nale Cosmetics - Enhance Your Natural Beauty" />
            <meta property="og:description" content="Shop high-quality, affordable personal care products made in Tamil Nadu, India." />
            <meta property="og:type" content="website" />
            <meta property="og:url" content="https://nalehcosmetics.com" />
            <meta property="og:image" content="static/img/logo.webp" />

            <style>{}</style>
            <style>{}</style>
        </head>
        <body>
            <nav>
                <ul>
                    <li><a href="/home"><img src="static/img/home.svg" alt="Home"><span>Home</span></a></li>
                    <li><a href="/services"><img src="static/img/cart.svg" alt="Services"><span>Shop</span></a></li>
                    <li><a href="/about"><img src="static/img/about.svg" alt="About"><span>About</span></a></li>
                </ul>
            </nav>
        {}
        </body>
        </html>
        "#,
        page,
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
    let file_path = PathBuf::from(format!("{}/{}", DATABASE_PATH, filename));

    let pass = query.get("pass").unwrap_or(&"".to_string()).clone();

    if pass == PASS.unwrap() {
        if file_path.exists() {
            match fs::remove_dir_all(&file_path) {
                Ok(_) => HttpResponse::Ok().body(format!("{} deleted successfully", filename)),
                Err(err) => {
                    println!("{:?}: {}", file_path, err);
                    HttpResponse::InternalServerError().body("Failed to delete file")
                }
            }
        } else {
            HttpResponse::NotFound().body("File not found")
        }
    } else {
        HttpResponse::NotFound().body("unauthorized access")
    }
}

async fn get_item(query: web::Query<std::collections::HashMap<String, String>>) -> impl Responder {
    let filename = query.get("item").unwrap_or(&"".to_string()).clone();
    let file_path = PathBuf::from(format!("{}/{}/user.json", DATABASE_PATH, filename));

    if file_path.exists() {
        match fs::read_to_string(&file_path) {
            Ok(file) => HttpResponse::Ok().json(json!({ "data": file })),
            Err(err) => {
                println!("{:?}: {}", file_path, err);
                HttpResponse::InternalServerError().body("Failed to delete file")
            }
        }
    } else {
        HttpResponse::NotFound().body("File not found")
    }
}

async fn entry() -> impl Responder {
    HttpResponse::NotFound().body("unauthorized access")
}

#[derive(Debug, Serialize, Deserialize)]
struct SanitizedFormData {
    id: String,
    firstname: String,
    description: String,
    price: i32,
    details: String,
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
                    println!("{:?}", json_string);
                    let mut json_value: Value = serde_json::from_str(&json_string).unwrap();
                    json_value["id"] = Value::String(Uuid::new_v4().to_string());
                    data = serde_json::from_value(json_value).unwrap();
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
                    let data = compress_image(data).unwrap();
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
    id: String,
    firstname: String,
    description: String,
    price: i32,
    details: String,
    rank: i32,
    url: String,
    images: Vec<String>,
}

impl Prodects {
    fn from(data: SanitizedFormData, images: Vec<String>) -> Self {
        Self {
            url: data.url,
            id: data.id,
            firstname: data.firstname.clone(),
            description: data.description,
            price: data.price,
            details: data.details,
            rank: data.rank,
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

async fn download_db() -> impl Responder {
    let mut buffer = Vec::new();
    {
        let mut tar_builder = tar::Builder::new(&mut buffer);

        let base_path = std::path::Path::new(DATABASE_PATH);

        add_dir_contents(&mut tar_builder, base_path, base_path).unwrap();

        tar_builder.finish().unwrap();
    }

    HttpResponse::Ok()
        .content_type("application/x-tar")
        .append_header(("Content-Disposition", "attachment; filename=\"db.tar\""))
        .body(buffer)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EmailSubmission {
    pub name: String,
    pub email: String,
    pub subject: String,
    pub message: String,
}

async fn submit_email(payload: web::Json<EmailSubmission>) -> impl Responder {
    send_mail(&payload).await.unwrap();
    HttpResponse::Ok().body("body")
}

const SMTP_USERNAME: Option<&str> = option_env!("SMTP_USERNAME");
const SMTP_PASSWORD: Option<&str> = option_env!("SMTP_PASSWORD");

pub async fn send_mail(user: &EmailSubmission) -> Result<(), Box<dyn Error>> {
    let email = Message::builder()
        .from(format!("nalehcosmetics.com website support <{}>", user.email).parse()?)
        .to(format!("Nalehcosmetics <nalehcosmetics@gmail.com>").parse()?)
        .subject(format!(
            "Support mail from nalehcosmetics.com. User: {}",
            user.name
        ))
        .header(ContentType::TEXT_PLAIN)
        .body(format!("{}", user.message))?;

    let creds = Credentials::new(
        SMTP_USERNAME.unwrap().to_owned(),
        SMTP_PASSWORD.unwrap().to_owned(),
    );

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")?
            .credentials(creds)
            .build();

    mailer.send(email).await?;
    Ok(())
}

fn add_dir_contents(
    tar: &mut tar::Builder<&mut Vec<u8>>,
    path: &std::path::Path,
    base: &std::path::Path,
) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                add_dir_contents(tar, &path, base)?;
            } else {
                let file_path = path.strip_prefix(base).unwrap();
                tar.append_path_with_name(&path, file_path)?;
            }
        }
    }
    Ok(())
}

fn compress_image(input: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let img = ImageReader::new(Cursor::new(input))
        .with_guessed_format()?
        .decode()?;

    // Resize (adjust size as needed)
    let resized = img.resize(800, 600, FilterType::Lanczos3);

    // Write compressed JPEG to buffer
    let mut out_buf = Cursor::new(Vec::new());
    resized.write_to(&mut out_buf, image::ImageFormat::WebP)?;

    Ok(out_buf.into_inner())
}

fn create_req_dir() -> Result<(), io::Error> {
    fs::create_dir_all(DATABASE_PATH)?;
    Ok(())
}

use env_logger::{self, Env};
use log::{error, info};
use zip::ZipArchive;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().filter_or("LOG", "info")).init();
    match create_req_dir() {
        Ok(_) => (),
        Err(err) => {
            error!("Error creating required path : {}", err)
        }
    };

    info!("Starting server on http://127.0.0.1:8081");

    HttpServer::new(|| {
        App::new()
            .service(afs::Files::new("/database", "./database").show_files_listing())
            .service(afs::Files::new("/static", "static").show_files_listing())
            .route("/files", web::get().to(list_files))
            .route("/get-item", web::get().to(get_item))
            .route("/delete/{filename}", web::delete().to(delete_file))
            .route("/entry", web::get().to(entry))
            .route("/download_db", web::get().to(download_db))
            .route("/submite", web::post().to(submit))
            .route("/api/products", web::get().to(get_products))
            .route("/submit_email", web::post().to(submit_email))
            .route("/{filename}", web::get().to(serve_page))
            .route("/", web::get().to(serve_page))
    })
    .bind("0.0.0.0:8081")?
    .run()
    .await
}
