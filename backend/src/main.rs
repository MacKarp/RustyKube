use std::env;
use std::str::FromStr;

use mongodb::options::{ClientOptions, FindOneOptions};
use mongodb::{
    bson,
    bson::{doc, oid::ObjectId, Bson, Document},
};
use mongodb::{Client, Collection};

use rocket::futures::StreamExt;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;

#[macro_use]
extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct Book {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub title: String,
    pub author: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
#[serde(rename_all = "camelCase")]
pub struct NewBook {
    pub title: String,
    pub author: String,
}

impl From<Book> for Document {
    fn from(book: Book) -> Self {
        bson::to_document(&book).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BookResponse {
    hostname: String,
    books: Vec<Book>,
}

#[get("/books")]
async fn get_books(collection: &State<Collection<Book>>) -> Json<BookResponse> {
    let mut cursor = collection.find(None, None).await.unwrap();
    let mut books = Vec::<Book>::new();
    while let Some(book) = cursor.next().await {
        match book {
            Ok(book) => books.push(book),
            Err(e) => println!("Error: {:?}", e),
        }
    }
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
    let response = BookResponse { hostname, books };
    Json(response)
}

#[get("/books/<id>")]
async fn get_book(id: &str, collection: &State<Collection<Book>>) -> Json<BookResponse> {
    let book = get_book_by_id_from_db(id, collection).await;
    let books = vec![book];
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
    let response = BookResponse { hostname, books };
    Json(response)
}

async fn get_book_by_id_from_db(id: &str, collection: &State<Collection<Book>>) -> Book {
    let oid = ObjectId::from_str(id).unwrap();
    let filter = doc! {"_id" : Bson::ObjectId(oid)};
    let options = FindOneOptions::builder().build();
    collection
        .find_one(filter.clone(), options)
        .await
        .unwrap()
        .unwrap()
}

#[post("/books", format = "json", data = "<book>")]
async fn post_book(
    book: Json<NewBook>,
    collection: &State<Collection<Book>>,
) -> Json<BookResponse> {
    let new_book = Book {
        id: ObjectId::new(),
        title: book.title.clone(),
        author: book.author.clone(),
    };
    let result = collection.insert_one(new_book, None).await.unwrap();

    let id = result.inserted_id.as_object_id().unwrap().to_string();
    let book = get_book_by_id_from_db(&id, collection).await;
    let books = vec![book];
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
    let response = BookResponse { hostname, books };
    Json(response)
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let mongo_user_name = env::var("MONGO_USER_NAME").unwrap();
    let mongo_user_password = env::var("MONGO_USER_PASSWORD").unwrap();
    let mongo_uri = env::var("MONGO_URI").unwrap();
    let database_name = env::var("DATABASE_NAME").unwrap();
    let mongo_app_name = env::var("MONGO_APP_NAME").unwrap();

    let mongo_full_uri =
        format!("mongodb+srv://{mongo_user_name}:{mongo_user_password}@{mongo_uri}");

    let mut client_options = ClientOptions::parse(mongo_full_uri).await.unwrap();
    client_options.app_name = Some(mongo_app_name.to_string());
    let client = Client::with_options(client_options).unwrap();
    println!("Trying to connect");
    client
        .database(&database_name)
        .run_command(doc! {"ping":1}, None)
        .await
        .unwrap();
    println!("Connected successfully.");

    let db = client.database(&database_name);

    let typed_collection = db.collection::<Book>("books");

    let _rocket = rocket::build()
        .manage(typed_collection)
        .mount("/", routes![index, get_books, get_book, post_book])
        .launch()
        .await?;
    Ok(())
}
