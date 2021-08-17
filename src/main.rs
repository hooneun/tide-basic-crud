use sqlx::{
    postgres::{PgPool},
    Pool,
};
use tide::{Body, Request, Response, Server};
// use sqlx::{query, query_as};
use dotenv;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug)]
struct State {
    db_pool: PgPool,
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
    let db_pool = make_db_pool().await;
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

pub async fn make_db_pool() -> PgPool {
    let db_url = std::env::var("DATABASE_URL").unwrap();
    Pool::connect(&db_url).await.unwrap()
}

async fn server(db_pool: PgPool) -> Server<State> {
    let state = State { db_pool };

    let dinos_endpoint = RestEntity {
        base_path: String::from("/dinos"),
    };

    let mut app = tide::with_state(state);
    app.at("/").get(|_| async { Ok("Ok") });

    register_rest_entity(&mut app, dinos_endpoint);

    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn index_page() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request as httpRequest, Response, Url};

        let db_pool = make_db_pool().await;
        let app = server(db_pool).await;
        let url = Url::parse("http://127.0.0.1:8080/").unwrap();
        let req = httpRequest::new(Method::Get, url);
        let mut res: Response = app.respond(req).await?;

        assert_eq!("Ok", res.body_string().await?);
        Ok(())
    }

    #[async_std::test]
    async fn create_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request, Response, Url};

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool().await;
        let app = server(db_pool).await;

        let url = Url::parse("http://127.0.0.1/dinos").unwrap();
        let mut req = Request::new(Method::Post, url);
        req.set_body(serde_json::to_string(&dino)?);
        let res: Response = app.respond(req).await?;
        assert_eq!(201, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn list_dinos() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request, Response, Url};

        let db_pool = make_db_pool().await;
        let app = server(db_pool).await;

        let url = Url::parse("http://127.0.0.1:8080/dinos").unwrap();
        let req = Request::new(Method::Get, url);
        let res: Response = app.respond(req).await?;

        assert_eq!(200, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn get_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request, Response, Url};

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("test_get"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool().await;
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
        let url =
            Url::parse(format!("http://127.0.0.1/dinos/{}", &dino.id.unwrap()).as_str()).unwrap();
        let req = Request::new(Method::Get, url);
        let res: Response = app.respond(req).await?;
        assert_eq!(200, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn delete_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request, Response, Url};

        let dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };

        let db_pool = make_db_pool().await;
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
        let url = Url::parse(format!("http://127.0.0.1:8080/dinos/{}", &dino.id.unwrap()).as_str())
            .unwrap();
        let req = Request::new(Method::Delete, url);
        let res: Response = app.respond(req).await?;

        assert_eq!(204, res.status());
        Ok(())
    }

    #[async_std::test]
    async fn update_dino() -> tide::Result<()> {
        dotenv::dotenv().ok();
        use tide::http::{Method, Request, Response, Url};

        let mut dino = Dino {
            id: Some(Uuid::new_v4()),
            name: String::from("update_test"),
            weight: 50,
            diet: String::from("carnivorous"),
        };
        let db_pool = make_db_pool().await;
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
        let url = Url::parse(format!("http://127.0.0.1:8080/dinos/{}", &dino.id.unwrap()).as_str()).unwrap();
        let mut req = Request::new(Method::Put, url);
        dino.weight = 100;
        req.set_body(serde_json::to_string(&dino)?);
        let res: Response = app.respond(req).await?;

        assert_eq!(200, res.status());
        assert_eq!(dino.weight, 100);

        Ok(())
    }
}
