use dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPool, Pool};
use tera::Tera;
use tide::{Body, Request, Response, Server};
use uuid::Uuid;

mod controller;
mod handlers;

use controller::{dino, views};

#[derive(Clone, Debug)]
struct State {
    db_pool: PgPool,
    tera: Tera,
}

#[derive(Debug, Deserialize, Serialize, Clone, sqlx::FromRow)]
struct Dino {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
    name: String,
    weight: i32,
    diet: String,
}

struct RestEntity {
    base_path: String,
}

impl RestEntity {
    async fn create(mut req: Request<State>) -> tide::Result {
        let dino: Dino = req.body_json().await?;
        let db_pool = req.state().db_pool.clone();
        let row = sqlx::query_as::<_, Dino>(
            "INSERT INTO dinos (id, name, weight, diet) VALUES
                 ($1, $2, $3, $4) returning id, name, weight, diet",
        )
        .bind(dino.id)
        .bind(dino.name)
        .bind(dino.weight)
        .bind(dino.diet)
        .fetch_one(&db_pool)
        .await?;

        let mut res = Response::new(201);
        res.set_body(Body::from_json(&row)?);

        Ok(res)
    }

    async fn list(req: Request<State>) -> tide::Result {
        let db_pool = req.state().db_pool.clone();
        let rows = sqlx::query_as::<_, Dino>("SELECT id, name, weight, diet from dinos")
            .fetch_all(&db_pool)
            .await?;

        let mut res = Response::new(200);
        res.set_body(Body::from_json(&rows)?);
        Ok(res)
    }

    async fn get(req: Request<State>) -> tide::Result {
        let db_pool = req.state().db_pool.clone();
        let id: Uuid = Uuid::parse_str(req.param("id")?).unwrap();
        let row =
            sqlx::query_as::<_, Dino>("SELECT id, name, weight, diet from dinos WHERE id = $1")
                .bind(id)
                .fetch_optional(&db_pool)
                .await?;

        let res = match row {
            None => Response::new(404),
            Some(row) => {
                let mut r = Response::new(200);
                r.set_body(Body::from_json(&row)?);
                r
            }
        };

        Ok(res)
    }

    async fn update(mut req: Request<State>) -> tide::Result {
        let dino: Dino = req.body_json().await?;
        let db_pool = req.state().db_pool.clone();
        let id: Uuid = Uuid::parse_str(req.param("id")?).unwrap();
        let row = sqlx::query_as::<_, Dino>(
            "UPDATE dinos SET name = $2, weight = $3, diet = $4
                WHERE id = $1
                returning id, name, weight, diet
                ",
        )
        .bind(id)
        .bind(dino.name)
        .bind(dino.weight)
        .bind(dino.diet)
        .fetch_optional(&db_pool)
        .await?;

        let res = match row {
            None => Response::new(404),
            Some(row) => {
                let mut r = Response::new(200);
                r.set_body(Body::from_json(&row)?);
                r
            }
        };

        Ok(res)
    }

    async fn delete(req: Request<State>) -> tide::Result {
        let db_pool = req.state().db_pool.clone();
        let id: Uuid = Uuid::parse_str(req.param("id")?).unwrap();
        let row = sqlx::query(
            "DELETE FROM dinos
                WHERE id = $1
                returning id",
        )
        .bind(id)
        .fetch_optional(&db_pool)
        .await?;

        let res = match row {
            None => Response::new(404),
            Some(_) => Response::new(204),
        };

        Ok(res)
    }
}

#[async_std::main]
async fn main() {
    dotenv::dotenv().ok();

    tide::log::start();
    let db_url = std::env::var("DATABASE_URL").unwrap();
    let db_pool = make_db_pool(&db_url).await;
    let app = server(db_pool).await;

    app.listen("127.0.0.1:8080").await.unwrap();
}

fn register_rest_entity(app: &mut Server<State>, entity: RestEntity) {
    app.at(&entity.base_path)
        .get(RestEntity::list)
        .post(RestEntity::create);

    println!("{}/:id", &entity.base_path);
    app.at(&format!("{}/:id", &entity.base_path))
        .get(RestEntity::get)
        .put(RestEntity::update)
        .delete(RestEntity::delete);
}

pub async fn make_db_pool(db_url: &str) -> PgPool {
    Pool::connect(&db_url).await.unwrap()
}

async fn server(db_pool: PgPool) -> Server<State> {
    let mut tera = Tera::new("templates/**/*").expect("Error parsing templates directory");
    tera.autoescape_on(vec!["html"]);

    let state = State { db_pool, tera };

    let dinos_endpoint = RestEntity {
        base_path: String::from("/dinos"),
    };

    let mut app = tide::with_state(state);
    app.at("/public")
        .serve_dir("./public/")
        .expect("Invalid static file directory");

    app.at("/").get(views::index);
    app.at("/dinos/new").get(views::new);
    app.at("/dinos").get(dino::list).post(dino::create);

    app.at("/dinos/:id/edit")
        .get(dino::get)
        .put(dino::update)
        .delete(dino::delete);

    register_rest_entity(&mut app, dinos_endpoint);

    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref DB_URL: String =
            std::env::var("DATABASE_URL_TEST").expect("missing env var DATABASE_URL_TEST");
    }

    async fn clear_dinos() -> Result<(), Box<dyn std::error::Error>> {
        let db_pool = make_db_pool(&DB_URL).await;

        sqlx::query("DELETE FROM dinos").execute(&db_pool).await?;
        Ok(())
    }

    #[async_std::test]
    async fn list_dinos() -> tide::Result<()> {
        dotenv::dotenv().ok();
        clear_dinos()
            .await
            .expect("Failed to clear the dinos table");

        let db_pool = make_db_pool(&DB_URL).await;
        let app = server(db_pool).await;

        let res = surf::Client::with_http_client(app)
            .get("http://127.0.0.1:8080/dinos")
            .await?;

        assert_eq!(200, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn create_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        clear_dinos()
            .await
            .expect("Failed to  clear the dinos table");
        use assert_json_diff::assert_json_eq;

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool(&DB_URL).await;
        let app = server(db_pool).await;

        let mut res = surf::Client::with_http_client(app)
            .post("http://127.0.0.1/dinos")
            .body(serde_json::to_string(&dino)?)
            .await?;

        assert_eq!(201, res.status());
        let d: Dino = res.body_json().await?;
        assert_json_eq!(dino, d);
        Ok(())
    }

    //    #[async_std::test]
    //    async fn create_dino_with_existing_key() -> tide::Result<()> {
    //        dotenv::dotenv().ok();
    //        clear_dinos()
    //            .await
    //            .expect("Failed to clear the dinos table");
    //
    //        let dino = Dino {
    //            id: Some(Uuid::new_v4()),
    //            name: String::from("test_get"),
    //            weight: 500,
    //            diet: String::from("carnivorous"),
    //        };
    //    }

    #[async_std::test]
    async fn get_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        clear_dinos()
            .await
            .expect("Failed to clear the dinos table");
        use assert_json_diff::assert_json_eq;

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("test_get"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool(&DB_URL).await;
        sqlx::query_as::<_, Dino>(
            "INSERT INTO dinos (id, name, weight, diet) VALUES
            ($1, $2, $3, $4) returning id, name, weight, diet",
        )
        .bind(&dino.id)
        .bind(&dino.name)
        .bind(&dino.weight)
        .bind(&dino.diet)
        .fetch_one(&db_pool)
        .await?;

        let app = server(db_pool).await;
        let mut res = surf::Client::with_http_client(app)
            .get(format!("http://127.0.0.1/dinos/{}", &dino.id.unwrap()).as_str())
            .await?;

        assert_eq!(200, res.status());

        let d: Dino = res.body_json().await?;
        assert_json_eq!(dino, d);
        Ok(())
    }

    #[async_std::test]
    async fn delete_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        clear_dinos()
            .await
            .expect("Failed to clear the dinos table");

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("delete_test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool(&DB_URL).await;
        sqlx::query_as::<_, Dino>(
            "INSERT INTO dinos (id, name, weight, diet) VALUES
            ($1, $2, $3, $4) returning id, name, weight, diet",
        )
        .bind(dino.id)
        .bind(dino.name)
        .bind(dino.weight)
        .bind(dino.diet)
        .fetch_one(&db_pool)
        .await?;

        let app = server(db_pool).await;
        let res = surf::Client::with_http_client(app)
            .delete(format!("http://127.0.0.1/dinos/{}", &dino.id.unwrap()))
            .await?;

        assert_eq!(204, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn update_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        clear_dinos()
            .await
            .expect("Failed to clear the dinos table");
        use assert_json_diff::assert_json_eq;

        let mut dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("update_test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };
        let db_pool = make_db_pool(&DB_URL).await;
        sqlx::query_as::<_, Dino>(
            "INSERT INTO dinos (id, name, weight, diet) VALUES
            ($1, $2, $3, $4) returning id, name, weight, diet",
        )
        .bind(&dino.id)
        .bind(&dino.name)
        .bind(&dino.weight)
        .bind(&dino.diet)
        .fetch_one(&db_pool)
        .await?;

        dino.name = String::from("updated from test");

        let app = server(db_pool).await;
        let mut res = surf::Client::with_http_client(app)
            .put(format!("http://127.0.0.1:8080/dinos/{}", &dino.id.unwrap()))
            .body(serde_json::to_string(&dino)?)
            .await?;
        assert_eq!(200, res.status());

        let d: Dino = res.body_json().await?;
        assert_json_eq!(dino, d);
        Ok(())
    }
}
